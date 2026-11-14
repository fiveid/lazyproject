use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;

use crate::git::{GitInfo, inspect_git};
use crate::toolchain::{ToolchainInfo, inspect_toolchain};

const RECLAIMABLE_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".venv",
    "venv",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".turbo",
    "__pycache__",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub path: PathBuf,
    pub git: Option<GitInfo>,
    pub toolchain: ToolchainInfo,
    pub last_activity_ts: i64,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub root: PathBuf,
    pub git_projects: Vec<ProjectEntry>,
    pub non_git_projects: Vec<ProjectEntry>,
}

fn modified_secs(path: &Path) -> Option<i64> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dur = mtime.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(dur.as_secs()).ok()
}

/// Compute activity timestamp without touching `.git/index` (which `git status` updates).
pub fn compute_activity_ts(project_path: &Path, git: Option<&GitInfo>) -> i64 {
    if let Some(g) = git {
        let mut latest = g.last_commit_ts.unwrap_or(0);
        for rel in &g.dirty_paths {
            let candidate = project_path.join(rel);
            if let Some(ts) = modified_secs(&candidate) {
                latest = latest.max(ts);
            }
        }
        return latest;
    }

    // Non-git directory: max of depth-1 non-hidden entry mtimes, or directory mtime.
    let mut latest = modified_secs(project_path).unwrap_or(0);
    if let Ok(entries) = fs::read_dir(project_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy() == ".DS_Store" {
                continue;
            }
            if let Some(ts) = modified_secs(&entry.path()) {
                latest = latest.max(ts);
            }
        }
    }
    latest
}

/// Compute total and safely reclaimable disk usage for a project directory (run in background).
pub fn compute_disk_usage(root: &Path) -> DiskUsage {
    fn walk(dir: &Path, inside_reclaimable: bool, usage: &mut DiskUsage) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                let name = entry.file_name();
                let is_reclaimable = inside_reclaimable
                    || RECLAIMABLE_DIRS.contains(&name.to_string_lossy().as_ref());
                walk(&entry.path(), is_reclaimable, usage);
            } else if meta.is_file() {
                let len = meta.len();
                usage.total_bytes = usage.total_bytes.saturating_add(len);
                if inside_reclaimable {
                    usage.reclaimable_bytes = usage.reclaimable_bytes.saturating_add(len);
                }
            }
        }
    }

    let mut usage = DiskUsage::default();
    walk(root, false, &mut usage);
    usage
}

pub fn inspect_single_project(path: PathBuf) -> ProjectEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let is_git = path.join(".git").exists();
    let git = if is_git {
        Some(inspect_git(&path))
    } else {
        None
    };

    let last_activity_ts = compute_activity_ts(&path, git.as_ref());
    let toolchain = inspect_toolchain(&path);

    ProjectEntry {
        name,
        path,
        git,
        toolchain,
        last_activity_ts,
    }
}

pub fn sort_projects(projects: &mut [ProjectEntry]) {
    projects.sort_by(|a, b| {
        b.last_activity_ts
            .cmp(&a.last_activity_ts)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub fn scan_workspace(root: &Path) -> Result<ScanResult, String> {
    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let entries = fs::read_dir(&canonical_root)
        .map_err(|e| format!("failed to read directory {}: {e}", canonical_root.display()))?;

    let mut candidate_dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Skip hidden directories (.git, .DS_Store, .cache, etc.)
        if name_str.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            candidate_dirs.push(path);
        }
    }

    let inspected: Vec<ProjectEntry> = candidate_dirs
        .into_par_iter()
        .map(inspect_single_project)
        .collect();

    let mut git_projects = Vec::new();
    let mut non_git_projects = Vec::new();

    for proj in inspected {
        if proj.git.is_some() {
            git_projects.push(proj);
        } else {
            non_git_projects.push(proj);
        }
    }

    sort_projects(&mut git_projects);
    sort_projects(&mut non_git_projects);

    Ok(ScanResult {
        root: canonical_root,
        git_projects,
        non_git_projects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sorts_by_most_recent_activity_then_name() {
        let mut items = vec![
            ProjectEntry {
                name: "zebra".into(),
                path: PathBuf::from("/tmp/zebra"),
                git: None,
                toolchain: ToolchainInfo::default(),
                last_activity_ts: 100,
            },
            ProjectEntry {
                name: "alpha".into(),
                path: PathBuf::from("/tmp/alpha"),
                git: None,
                toolchain: ToolchainInfo::default(),
                last_activity_ts: 100,
            },
            ProjectEntry {
                name: "newest".into(),
                path: PathBuf::from("/tmp/newest"),
                git: None,
                toolchain: ToolchainInfo::default(),
                last_activity_ts: 500,
            },
        ];

        sort_projects(&mut items);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["newest", "alpha", "zebra"]);
    }

    #[test]
    fn skips_hidden_dirs_and_separates_non_git() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join(".hidden")).unwrap();
        fs::create_dir(tmp.path().join("plain-folder")).unwrap();
        fs::write(tmp.path().join("random-file.txt"), "hi").unwrap();

        let res = scan_workspace(tmp.path()).unwrap();
        assert!(res.git_projects.is_empty());
        assert_eq!(res.non_git_projects.len(), 1);
        assert_eq!(res.non_git_projects[0].name, "plain-folder");
    }

    #[test]
    fn computes_total_and_reclaimable_disk_usage() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("src.rs"), vec![b'a'; 100]).unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();
        fs::write(tmp.path().join("node_modules/pkg.js"), vec![b'b'; 250]).unwrap();

        let usage = compute_disk_usage(tmp.path());
        assert_eq!(usage.total_bytes, 350);
        assert_eq!(usage.reclaimable_bytes, 250);
    }
}
