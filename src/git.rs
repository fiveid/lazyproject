use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteProvider {
    GitHub,
    GitLab,
    Bitbucket,
    Other,
    None,
}

impl RemoteProvider {
    pub fn short_badge(&self) -> &'static str {
        match self {
            RemoteProvider::GitHub => "GH",
            RemoteProvider::GitLab => "GL",
            RemoteProvider::Bitbucket => "BB",
            RemoteProvider::Other => "GIT",
            RemoteProvider::None => "LOC",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RemoteProvider::GitHub => "GitHub",
            RemoteProvider::GitLab => "GitLab",
            RemoteProvider::Bitbucket => "Bitbucket",
            RemoteProvider::Other => "Git Remote",
            RemoteProvider::None => "Local Only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub short_sha: String,
    pub timestamp: i64,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub remote_url: Option<String>,
    pub provider: RemoteProvider,
    pub last_commit_ts: Option<i64>,
    pub recent_commits: Vec<CommitSummary>,
    pub error: Option<String>,
}

impl GitInfo {
    pub fn is_dirty(&self) -> bool {
        self.staged > 0 || self.modified > 0 || self.untracked > 0 || self.conflicted > 0
    }
}

/// Strip embedded credentials/userinfo from HTTP(S)/SSH URLs before storing or displaying.
pub fn sanitize_remote_url(raw: &str) -> String {
    let trimmed = raw.trim();
    for scheme in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            // Find '@' before the first '/'
            let slash_idx = rest.find('/').unwrap_or(rest.len());
            if let Some(at_idx) = rest[..slash_idx].find('@') {
                return format!("{scheme}{}", &rest[at_idx + 1..]);
            }
            return trimmed.to_string();
        }
    }
    trimmed.to_string()
}

/// Detect the hosting provider from a remote URL.
pub fn detect_provider(url: &str) -> RemoteProvider {
    let lower = url.to_ascii_lowercase();
    if lower.contains("github.com") {
        RemoteProvider::GitHub
    } else if lower.contains("gitlab.com") || lower.contains("gitlab.") {
        RemoteProvider::GitLab
    } else if lower.contains("bitbucket.org") {
        RemoteProvider::Bitbucket
    } else if lower.is_empty() {
        RemoteProvider::None
    } else {
        RemoteProvider::Other
    }
}

fn run_git_readonly(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("git exited with {}", output.status)
        } else {
            stderr
        })
    }
}

/// Parse `git status --porcelain=v2 --branch` output.
pub fn parse_porcelain_v2(output: &str, info: &mut GitInfo) {
    for line in output.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            if head == "(detached)" {
                info.detached = true;
                info.branch = "detached".to_string();
            } else {
                info.branch = head.to_string();
            }
        } else if let Some(upstream) = line.strip_prefix("# branch.upstream ") {
            info.upstream = Some(upstream.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for part in ab.split_whitespace() {
                if let Some(ahead) = part.strip_prefix('+') {
                    info.ahead = ahead.parse::<u32>().unwrap_or(0);
                } else if let Some(behind) = part.strip_prefix('-') {
                    info.behind = behind.parse::<u32>().unwrap_or(0);
                }
            }
        } else if line.starts_with("? ") {
            info.untracked += 1;
        } else if line.starts_with("u ") {
            info.conflicted += 1;
        } else if (line.starts_with("1 ") || line.starts_with("2 "))
            && let Some(xy) = line.split_whitespace().nth(1)
        {
            let mut chars = xy.chars();
            let x = chars.next().unwrap_or('.');
            let y = chars.next().unwrap_or('.');
            if x != '.' {
                info.staged += 1;
            }
            if y != '.' {
                info.modified += 1;
            }
        }
    }
}

fn read_remote_url(repo_path: &Path) -> Option<String> {
    if let Ok(url) = run_git_readonly(repo_path, &["remote", "get-url", "origin"]) {
        let s = sanitize_remote_url(&url);
        if !s.is_empty() {
            return Some(s);
        }
    }
    if let Ok(remotes) = run_git_readonly(repo_path, &["remote"])
        && let Some(first) = remotes.lines().next()
        && let Ok(url) = run_git_readonly(repo_path, &["remote", "get-url", first.trim()])
    {
        let s = sanitize_remote_url(&url);
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn read_recent_commits(repo_path: &Path) -> Vec<CommitSummary> {
    let Ok(out) = run_git_readonly(repo_path, &["log", "-n", "3", "--format=%h%x09%ct%x09%s"])
    else {
        return Vec::new();
    };

    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let sha = parts.next()?.trim().to_string();
            let ts = parts.next()?.trim().parse::<i64>().ok()?;
            let subject = parts.next().unwrap_or("").trim().to_string();
            Some(CommitSummary {
                short_sha: sha,
                timestamp: ts,
                subject,
            })
        })
        .collect()
}

/// Inspect a git repository strictly read-only.
pub fn inspect_git(repo_path: &Path) -> GitInfo {
    let mut info = GitInfo {
        branch: "unknown".to_string(),
        detached: false,
        upstream: None,
        ahead: 0,
        behind: 0,
        staged: 0,
        modified: 0,
        untracked: 0,
        conflicted: 0,
        remote_url: None,
        provider: RemoteProvider::None,
        last_commit_ts: None,
        recent_commits: Vec::new(),
        error: None,
    };

    match run_git_readonly(repo_path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(out) => parse_porcelain_v2(&out, &mut info),
        Err(err) => {
            info.error = Some(err);
            return info;
        }
    }

    info.remote_url = read_remote_url(repo_path);
    info.provider = match &info.remote_url {
        Some(url) => detect_provider(url),
        None => RemoteProvider::None,
    };

    info.recent_commits = read_recent_commits(repo_path);
    info.last_commit_ts = info.recent_commits.first().map(|c| c.timestamp);

    info
}

/// Explicitly fetch remotes for a repository (only called on user action).
pub fn fetch_remote(repo_path: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["fetch", "--prune", "--no-tags"])
        .output()
        .map_err(|e| format!("failed to run git fetch: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("git fetch exited with {}", output.status)
        } else {
            stderr
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_https_credentials() {
        assert_eq!(
            sanitize_remote_url("https://user:ghp_secret123@github.com/org/repo.git\n"),
            "https://github.com/org/repo.git"
        );
        assert_eq!(
            sanitize_remote_url("https://oauth2:glpat-xyz@gitlab.com/team/app.git"),
            "https://gitlab.com/team/app.git"
        );
        assert_eq!(
            sanitize_remote_url("https://github.com/org/repo.git"),
            "https://github.com/org/repo.git"
        );
        assert_eq!(
            sanitize_remote_url("git@github.com:org/repo.git"),
            "git@github.com:org/repo.git"
        );
    }

    #[test]
    fn detects_providers() {
        assert_eq!(
            detect_provider("git@github.com:user/repo.git"),
            RemoteProvider::GitHub
        );
        assert_eq!(
            detect_provider("https://gitlab.com/group/proj.git"),
            RemoteProvider::GitLab
        );
        assert_eq!(
            detect_provider("https://gitlab.internal.corp/team/proj.git"),
            RemoteProvider::GitLab
        );
        assert_eq!(
            detect_provider("https://bitbucket.org/team/proj.git"),
            RemoteProvider::Bitbucket
        );
        assert_eq!(
            detect_provider("https://git.sr.ht/~user/repo"),
            RemoteProvider::Other
        );
        assert_eq!(detect_provider(""), RemoteProvider::None);
    }

    #[test]
    fn parses_porcelain_v2_status() {
        let sample = "\
# branch.oid 1234567890abcdef
# branch.head feat/tui
# branch.upstream origin/feat/tui
# branch.ab +3 -2
1 .M N... 100644 100644 100644 abc def src/main.rs
1 A. N... 000000 100644 100644 000 abc src/new.rs
1 MM N... 100644 100644 100644 abc def Cargo.toml
u UU N... 100644 100644 100644 100644 a b c conflict.txt
? untracked.txt
";
        let mut info = GitInfo {
            branch: String::new(),
            detached: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            staged: 0,
            modified: 0,
            untracked: 0,
            conflicted: 0,
            remote_url: None,
            provider: RemoteProvider::None,
            last_commit_ts: None,
            recent_commits: Vec::new(),
            error: None,
        };
        parse_porcelain_v2(sample, &mut info);
        assert_eq!(info.branch, "feat/tui");
        assert_eq!(info.upstream.as_deref(), Some("origin/feat/tui"));
        assert_eq!(info.ahead, 3);
        assert_eq!(info.behind, 2);
        assert_eq!(info.staged, 2);
        assert_eq!(info.modified, 2);
        assert_eq!(info.conflicted, 1);
        assert_eq!(info.untracked, 1);
        assert!(info.is_dirty());
    }
}
