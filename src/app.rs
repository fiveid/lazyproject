use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rayon::prelude::*;

use crate::git::fetch_remote;
use crate::scanner::{ProjectEntry, ScanResult, inspect_single_project, scan_workspace};
use crate::terminal::open_in_new_tab;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Git(usize),
    NonGitToggle,
    NonGit(usize),
}

pub enum BgMessage {
    RescanDone(Result<ScanResult, String>),
    FetchSingleDone {
        path: PathBuf,
        result: Result<Box<ProjectEntry>, String>,
    },
    FetchAllDone {
        succeeded: usize,
        failed: usize,
        scan: Result<ScanResult, String>,
    },
}

pub struct AppState {
    pub root: PathBuf,
    pub git_projects: Vec<ProjectEntry>,
    pub non_git_projects: Vec<ProjectEntry>,
    pub non_git_expanded: bool,
    pub selected: usize,
    pub filter_active: bool,
    pub filter_query: String,
    pub show_help: bool,
    pub busy: bool,
    pub status_message: String,
    pub should_quit: bool,
    bg_tx: Sender<BgMessage>,
    bg_rx: Receiver<BgMessage>,
}

impl AppState {
    pub fn new(scan: ScanResult) -> Self {
        let (bg_tx, bg_rx) = mpsc::channel();
        let count = scan.git_projects.len();
        Self {
            root: scan.root,
            git_projects: scan.git_projects,
            non_git_projects: scan.non_git_projects,
            non_git_expanded: false,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            show_help: false,
            busy: false,
            status_message: format!("Loaded {count} git projects (read-only)"),
            should_quit: false,
            bg_tx,
            bg_rx,
        }
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

        let matching_non_git: Vec<usize> = self
            .non_git_projects
            .iter()
            .enumerate()
            .filter(|(_, p)| matches(p))
            .map(|(i, _)| i)
            .collect();

        if !matching_non_git.is_empty() {
            rows.push(DisplayRow::NonGitToggle);
            if self.non_git_expanded {
                for i in matching_non_git {
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
            DisplayRow::NonGitToggle => None,
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
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMessage::RescanDone(Ok(scan)) => {
                    self.git_projects = scan.git_projects;
                    self.non_git_projects = scan.non_git_projects;
                    self.busy = false;
                    self.clamp_selection();
                    self.status_message =
                        format!("Refreshed ({} git projects)", self.git_projects.len());
                }
                BgMessage::RescanDone(Err(err)) => {
                    self.busy = false;
                    self.status_message = format!("Refresh error: {err}");
                }
                BgMessage::FetchSingleDone { path, result } => {
                    self.busy = false;
                    match result {
                        Ok(updated) => {
                            let name = updated.name.clone();
                            if let Some(slot) =
                                self.git_projects.iter_mut().find(|p| p.path == path)
                            {
                                *slot = *updated;
                            }
                            self.status_message = format!("Fetched {name}");
                        }
                        Err(err) => {
                            self.status_message = format!("Fetch failed: {err}");
                        }
                    }
                }
                BgMessage::FetchAllDone {
                    succeeded,
                    failed,
                    scan,
                } => {
                    self.busy = false;
                    if let Ok(s) = scan {
                        self.git_projects = s.git_projects;
                        self.non_git_projects = s.non_git_projects;
                        self.clamp_selection();
                    }
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
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                if len > 0 {
                    self.selected = len - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let rows = self.display_rows();
                if matches!(rows.get(self.selected), Some(DisplayRow::NonGitToggle)) {
                    self.non_git_expanded = !self.non_git_expanded;
                    self.clamp_selection();
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
        self.status_message = format!("Fetching {name}...");
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let result =
                fetch_remote(&path).map(|()| Box::new(inspect_single_project(path.clone())));
            let _ = tx.send(BgMessage::FetchSingleDone { path, result });
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
        self.status_message = format!("Fetching {} repositories...", paths.len());
        let root = self.root.clone();
        let tx = self.bg_tx.clone();
        thread::spawn(move || {
            let results: Vec<Result<(), String>> =
                paths.par_iter().map(|p| fetch_remote(p)).collect();
            let succeeded = results.iter().filter(|r| r.is_ok()).count();
            let failed = results.len() - succeeded;
            let scan = scan_workspace(&root);
            let _ = tx.send(BgMessage::FetchAllDone {
                succeeded,
                failed,
                scan,
            });
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
    fn non_git_row_collapses_and_expands() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp"),
            git_projects: vec![dummy_proj("app", true)],
            non_git_projects: vec![dummy_proj("notes", false), dummy_proj("assets", false)],
        };
        let mut state = AppState::new(scan);
        assert_eq!(
            state.display_rows(),
            vec![DisplayRow::Git(0), DisplayRow::NonGitToggle]
        );

        state.selected = 1;
        state.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(state.non_git_expanded);
        assert_eq!(
            state.display_rows(),
            vec![
                DisplayRow::Git(0),
                DisplayRow::NonGitToggle,
                DisplayRow::NonGit(0),
                DisplayRow::NonGit(1),
            ]
        );
    }
}
