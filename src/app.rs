use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rayon::prelude::*;

use crate::git::fetch_remote;
use crate::scanner::{
    DiskUsage, ProjectEntry, ScanResult, compute_disk_usage, inspect_single_project, scan_workspace,
};
use crate::terminal::open_in_new_tab;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Git(usize),
    NonGit(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchState {
    Idle,
    Fetching,
    Done,
    Failed(String),
}

pub enum BgMessage {
    RescanDone(Result<ScanResult, String>),
    DiskUsageComputed {
        path: PathBuf,
        usage: DiskUsage,
    },
    FetchRepoStarted(PathBuf),
    FetchRepoFinished {
        path: PathBuf,
        result: Result<Box<ProjectEntry>, String>,
    },
    FetchAllComplete {
        succeeded: usize,
        failed: usize,
    },
}

pub struct AppState {
    pub root: PathBuf,
    pub git_projects: Vec<ProjectEntry>,
    pub non_git_projects: Vec<ProjectEntry>,
    pub show_non_git: bool,
    pub selected: usize,
    pub inspector_focused: bool,
    pub inspector_scroll: u16,
    pub filter_active: bool,
    pub filter_query: String,
    pub show_help: bool,
    pub busy: bool,
    pub tick: usize,
    pub status_message: String,
    pub should_quit: bool,
    pub fetch_states: HashMap<PathBuf, FetchState>,
    pub disk_usage: HashMap<PathBuf, DiskUsage>,
    bg_tx: Sender<BgMessage>,
    bg_rx: Receiver<BgMessage>,
}

impl AppState {
    pub fn new(scan: ScanResult) -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();
        let count = scan.git_projects.len();
        let state = Self {
            root: scan.root,
            git_projects: scan.git_projects,
            non_git_projects: scan.non_git_projects,
            show_non_git: false,
            selected: 0,
            inspector_focused: false,
            inspector_scroll: 0,
            filter_active: false,
            filter_query: String::new(),
            show_help: false,
            busy: false,
            tick: 0,
            status_message: format!("Loaded {count} git projects (read-only)"),
            should_quit: false,
            fetch_states: HashMap::new(),
            disk_usage: HashMap::new(),
            bg_tx,
            bg_rx,
        };
        state.spawn_disk_usage_scan();
        state
    }

    fn spawn_disk_usage_scan(&self) {
        let paths: Vec<PathBuf> = self
            .git_projects
            .iter()
            .chain(self.non_git_projects.iter())
            .map(|p| p.path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            paths.into_par_iter().for_each(|path| {
                let usage = compute_disk_usage(&path);
                let _ = tx.send(BgMessage::DiskUsageComputed { path, usage });
            });
        });
    }

    pub fn display_rows(&self) -> Vec<DisplayRow> {
        let q = self.filter_query.to_ascii_lowercase();
        let matches = |p: &ProjectEntry| q.is_empty() || p.name.to_ascii_lowercase().contains(&q);

        let mut rows = Vec::new();
        for (i, proj) in self.git_projects.iter().enumerate() {
            if matches(proj) {
                rows.push(DisplayRow::Git(i));
            }
        }

        if self.show_non_git {
            for (i, proj) in self.non_git_projects.iter().enumerate() {
                if matches(proj) {
                    rows.push(DisplayRow::NonGit(i));
                }
            }
        }

        rows
    }

    pub fn selected_project(&self) -> Option<&ProjectEntry> {
        let rows = self.display_rows();
        match rows.get(self.selected)? {
            DisplayRow::Git(i) => self.git_projects.get(*i),
            DisplayRow::NonGit(i) => self.non_git_projects.get(*i),
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.display_rows().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    pub fn poll_background(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMessage::RescanDone(Ok(scan)) => {
                    self.git_projects = scan.git_projects;
                    self.non_git_projects = scan.non_git_projects;
                    self.busy = false;
                    self.clamp_selection();
                    self.status_message =
                        format!("Refreshed ({} git projects)", self.git_projects.len());
                    self.spawn_disk_usage_scan();
                }
                BgMessage::RescanDone(Err(err)) => {
                    self.busy = false;
                    self.status_message = format!("Refresh error: {err}");
                }
                BgMessage::DiskUsageComputed { path, usage } => {
                    self.disk_usage.insert(path, usage);
                }
                BgMessage::FetchRepoStarted(path) => {
                    self.fetch_states.insert(path, FetchState::Fetching);
                }
                BgMessage::FetchRepoFinished { path, result } => {
                    match result {
                        Ok(updated) => {
                            let name = updated.name.clone();
                            if let Some(slot) =
                                self.git_projects.iter_mut().find(|p| p.path == path)
                            {
                                *slot = *updated;
                            }
                            self.fetch_states.insert(path, FetchState::Done);
                            if !self.busy {
                                self.status_message = format!("Fetched {name}");
                            }
                        }
                        Err(err) => {
                            self.fetch_states
                                .insert(path, FetchState::Failed(err.clone()));
                            if !self.busy {
                                self.status_message = format!("Fetch failed: {err}");
                            }
                        }
                    }
                    // If this was a single-repo fetch, busy clears when no repos are Fetching
                    if self.busy
                        && !self
                            .fetch_states
                            .values()
                            .any(|s| matches!(s, FetchState::Fetching))
                    {
                        self.busy = false;
                    }
                }
                BgMessage::FetchAllComplete { succeeded, failed } => {
                    self.busy = false;
                    self.status_message =
                        format!("Fetched all: {succeeded} succeeded, {failed} failed");
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return;
        }

        if self.filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.filter_active = false;
                    self.filter_query.clear();
                    self.clamp_selection();
                }
                KeyCode::Enter => {
                    self.filter_active = false;
                }
                KeyCode::Backspace => {
                    self.filter_query.pop();
                    self.clamp_selection();
                }
                KeyCode::Char(c) => {
                    self.filter_query.push(c);
                    self.clamp_selection();
                }
                _ => {}
            }
            return;
        }

        if self.inspector_focused {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Tab => {
                    self.inspector_focused = false;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.inspector_scroll = self.inspector_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.inspector_scroll = self.inspector_scroll.saturating_sub(1);
                }
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }

        let len = self.display_rows().len();
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => {
                self.filter_active = true;
            }
            KeyCode::Esc => {
                if !self.filter_query.is_empty() {
                    self.filter_query.clear();
                    self.clamp_selection();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if len > 0 && self.selected + 1 < len {
                    self.selected += 1;
                    self.inspector_scroll = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.inspector_scroll = 0;
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected = 0;
                self.inspector_scroll = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if len > 0 {
                    self.selected = len - 1;
                    self.inspector_scroll = 0;
                }
            }
            KeyCode::Char('n') => {
                self.show_non_git = !self.show_non_git;
                self.clamp_selection();
                self.status_message = if self.show_non_git {
                    format!(
                        "Showing {} non-git directories",
                        self.non_git_projects.len()
                    )
                } else {
                    format!("Hid {} non-git directories", self.non_git_projects.len())
                };
            }
            KeyCode::Enter | KeyCode::Tab => {
                if self.selected_project().is_some() {
                    self.inspector_focused = true;
                }
            }
            KeyCode::Char('o') => {
                if let Some(proj) = self.selected_project() {
                    match open_in_new_tab(&proj.path) {
                        Ok(msg) => self.status_message = msg,
                        Err(err) => self.status_message = format!("Open failed: {err}"),
                    }
                }
            }
            KeyCode::Char('r') => {
                self.trigger_rescan();
            }
            KeyCode::Char('f') => {
                self.trigger_fetch_selected();
            }
            KeyCode::Char('F') => {
                self.trigger_fetch_all();
            }
            _ => {}
        }
    }

    fn trigger_rescan(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status_message = "Refreshing workspace...".to_string();
        let root = self.root.clone();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let _ = tx.send(BgMessage::RescanDone(scan_workspace(&root)));
        });
    }

    fn trigger_fetch_selected(&mut self) {
        if self.busy {
            return;
        }
        let Some(proj) = self.selected_project() else {
            return;
        };
        if proj.git.is_none() {
            self.status_message = "Selected directory is not a git repository".to_string();
            return;
        }
        let path = proj.path.clone();
        let name = proj.name.clone();
        self.busy = true;
        self.fetch_states.insert(path.clone(), FetchState::Fetching);
        self.status_message = format!("Fetching {name}...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result =
                fetch_remote(&path).map(|()| Box::new(inspect_single_project(path.clone())));
            let _ = tx.send(BgMessage::FetchRepoFinished { path, result });
        });
    }

    fn trigger_fetch_all(&mut self) {
        if self.busy {
            return;
        }
        let paths: Vec<PathBuf> = self
            .git_projects
            .iter()
            .filter(|p| p.git.as_ref().is_some_and(|g| g.remote_url.is_some()))
            .map(|p| p.path.clone())
            .collect();
        if paths.is_empty() {
            self.status_message = "No git projects with remotes to fetch".to_string();
            return;
        }
        self.busy = true;
        for p in &paths {
            self.fetch_states.insert(p.clone(), FetchState::Fetching);
        }
        self.status_message = format!("Fetching {} repositories...", paths.len());
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let results: Vec<bool> = paths
                .into_par_iter()
                .map(|path| {
                    let _ = tx.send(BgMessage::FetchRepoStarted(path.clone()));
                    let res = fetch_remote(&path)
                        .map(|()| Box::new(inspect_single_project(path.clone())));
                    let ok = res.is_ok();
                    let _ = tx.send(BgMessage::FetchRepoFinished { path, result: res });
                    ok
                })
                .collect();
            let succeeded = results.iter().filter(|&&ok| ok).count();
            let failed = results.len() - succeeded;
            let _ = tx.send(BgMessage::FetchAllComplete { succeeded, failed });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolchain::ToolchainInfo;

    fn dummy_proj(name: &str, is_git: bool) -> ProjectEntry {
        ProjectEntry {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            git: if is_git {
                Some(crate::git::GitInfo {
                    branch: "main".into(),
                    detached: false,
                    upstream: None,
                    ahead: 0,
                    behind: 0,
                    staged: 0,
                    modified: 0,
                    untracked: 0,
                    conflicted: 0,
                    remote_url: None,
                    provider: crate::git::RemoteProvider::None,
                    last_commit_ts: None,
                    recent_commits: vec![],
                    dirty_paths: vec![],
                    error: None,
                })
            } else {
                None
            },
            toolchain: ToolchainInfo::default(),
            last_activity_ts: 100,
        }
    }

    #[test]
    fn n_key_toggles_non_git_projects_globally() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp"),
            git_projects: vec![dummy_proj("app", true)],
            non_git_projects: vec![dummy_proj("notes", false), dummy_proj("assets", false)],
        };
        let mut state = AppState::new(scan);
        assert_eq!(state.display_rows(), vec![DisplayRow::Git(0)]);

        state.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert!(state.show_non_git);
        assert_eq!(
            state.display_rows(),
            vec![
                DisplayRow::Git(0),
                DisplayRow::NonGit(0),
                DisplayRow::NonGit(1),
            ]
        );
    }
}
