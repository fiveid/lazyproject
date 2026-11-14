# lazyproject — v0.1.0 Specification

## 1. Overview

`lazyproject` is a fast, read-only-first Terminal UI (TUI) CLI tool written in Rust (in the style of `lazydocker` and `lazygit`) for getting an immediate overview of all projects inside a root directory.

Running `lazyproject` in any directory (or passing a path `lazyproject [PATH]`) treats that directory as the root workspace containing your projects. It surfaces uncommitted work, unpushed commits, behind-remote branches, remote hosting provider (GitHub, GitLab, etc.), and required toolchains/package managers — without mutating your repositories.

---

## 2. Core Principles & Security Invariants

1. **Read-Only by Default**:
   - Startup, scanning, sorting, and inspection MUST NEVER modify working trees, staging areas, git configs, or remote repositories.
   - `git fetch` is **never** run automatically on startup. It is only executed when the user explicitly presses the fetch key (`f` for selected repo, `F` for all repos).
   - No `git push`, `git commit`, `git checkout`, `git reset`, `git clean`, or destructive filesystem operations exist in `lazyproject`.
2. **No Sensitive Data Leakage**:
   - Remote URLs MUST have embedded credentials/tokens stripped before display (e.g. `https://user:token@github.com/...` -> `https://github.com/...`).
   - `.env` files and secret files are never read or displayed.
   - No telemetry, analytics, or outbound network calls other than explicit user-triggered `git fetch`.
3. **Subprocess & Path Safety**:
   - All external commands (`git`, `osascript`, `open`, `wezterm`, `kitty`) MUST be invoked with argument arrays (never `sh -c` string interpolation) to prevent command injection from malicious directory or branch names.
   - Symlinks and unreadable directories must be handled gracefully without panicking or infinite loops.

---

## 3. Project Discovery & Classification

- **Root Directory**: Defaults to current working directory (`std::env::current_dir()`), or optional CLI argument `lazyproject <PATH>`.
- **Scan Depth**: Immediate subdirectories (depth 1) of the root directory.
- **Hidden Directories**: Directories starting with `.` (e.g., `.git`, `.DS_Store`, `.cache`) are skipped.
- **Classification**:
  - **Git Project**: Depth-1 directory containing a `.git` entry (directory or file, supporting worktrees/submodules).
  - **Non-Git Directory**: Depth-1 non-hidden directory without `.git`.
- **Non-Git Collapsing**:
  - Non-git directories are hidden by default behind a single summary row at the bottom of the project list:
    `+ N projects not on Git (press Enter or Space to expand)`
  - Expanding reveals the non-git directories (with their last modified time and detected toolchain markers if any). Collapsing hides them again.

---

## 4. Sorting & Attention Flags

### 4.1 Sorting ("Most Recently Changed at the Top")
Git projects are sorted in descending order by **last activity timestamp**:
- **Clean git repos**: `HEAD` commit timestamp (never `.git/index` mtime, because running `git status` refreshes the index stat cache and would falsely mark untouched repos as modified "just now").
- **Dirty git repos**: `max(head_commit_timestamp, max_mtime_of_dirty_or_untracked_files)`.
- **Non-git directories**: `max` of depth-1 entry mtimes (excluding `.DS_Store`).
- Ties are broken alphabetically by directory name.

### 4.2 Attention Flags / Badges
Projects display clear visual indicators in the table for:
- **Dirty working tree**: compact counts (`+S` staged, `~M` modified, `?U` untracked, `!C` conflicted) or `✓` when clean.
- **Ahead of remote**: unpushed local commits (`↑N`) — *unbacked up locally*
- **Behind remote**: remote has commits not yet pulled (`↓N`) — *sync before editing*
- **No remote configured**: local-only git repo (`LOC`)
- **Detached HEAD**: (`detached`)
- **Remote Provider**: `GH` (GitHub), `GL` (GitLab), `BB` (Bitbucket), `GIT` (other), or `LOC` (local-only)
- **Live Fetch Status**: animated indicator while fetching (`⠋` / `●`), red dot (`●`) + error in Inspector if fetch failed.

### 4.3 Disk Usage & Safely Reclaimable Space (Background)
- Disk size and safely reclaimable dependency/build artifact size (`node_modules`, `target`, `.venv`, `venv`, `dist`, `build`, `.next`) are **never** computed synchronously on startup (to avoid slowing down initial render).
- Instead, a low-priority background pass streams sizes into the UI after startup, showing total size in the table and reclaimable breakdown in the Inspector. Display-only; `lazyproject` never deletes files.

---

## 5. Terminal UI (TUI) Layout & Keybindings

Two-pane layout (responsive to terminal width, with a bottom keybinding help bar):

1. **Left Pane — Project Table**:
   - Consistent aligned columns: `Host | Project | Branch | Sync | Tree | Size | Age`
   - Live fetch indicator (spinner while fetching, red dot on failure)
   - Search/filter indicator when filtering (`/`)
   - Non-git projects hidden by default with a footer/summary indicator (`+N non-git`), toggled globally with `n`.
2. **Right Pane — Project Inspector**:
   - **Summary**: Full path, last activity, remote host & sanitized URL.
   - **Git Status**: Branch, tracking upstream, ahead/behind counts, staged/modified/untracked/conflicted counts, and last 3 commits (short SHA, relative time, subject).
   - **Toolchain & Platform**:
     - **Mise**: Parsed from `mise.toml`, `.mise.toml`, or `.tool-versions` (shows tools & pinned versions, e.g., `node 22.1.0`, `rust 1.82.0`).
     - **Ecosystem & Package Manager**: Detected from manifest & lockfiles:
       - Node.js (`package.json`, `pnpm-lock.yaml` -> pnpm, `yarn.lock` -> yarn, `bun.lockb`/`bun.lock` -> bun, `package-lock.json` -> npm; plus `engines.node` / `packageManager` if present)
       - Rust (`Cargo.toml` / `Cargo.lock`)
       - Python (`pyproject.toml`, `uv.lock` -> uv, `poetry.lock` -> poetry, `requirements.txt` -> pip)
       - Go (`go.mod`)
3. **Bottom Bar — Keybindings & Status**:
   - Shows active filter, fetch progress/errors, or status messages (e.g., "Opened in new tab", "Fetched 12 repos").

### Keybindings (v0.1.0)
| Key | Action |
|---|---|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `g` / `Home` | Jump to top |
| `G` / `End` | Jump to bottom |
| `n` | Toggle showing/hiding non-Git directories |
| `Enter` | Toggle focus / scroll Inspector pane |
| `o` | Open selected project directory in a new terminal tab |
| `f` | Fetch remote for selected git project (`git fetch --prune --no-tags` or `git fetch`) |
| `F` | Fetch remotes for all git projects (concurrently) |
| `r` | Refresh / rescan workspace |
| `/` | Filter projects by name |
| `Esc` | Clear filter / close help |
| `?` | Toggle keybindings help overlay |
| `q` / `Ctrl+C` | Quit |

---

## 6. Opening a Project in a New Terminal Tab (macOS)

Auto-detects `$TERM_PROGRAM` (zero user setup):
- **Ghostty** (`TERM_PROGRAM=ghostty`): Uses `open -na Ghostty --args --working-directory=<dir>` or Ghostty tab/window launch (note: Ghostty on macOS does not yet expose an AppleScript `new tab` command without Accessibility permissions; `open -na Ghostty --args --working-directory=<dir>` opens cleanly without requiring Accessibility permissions).
- **Terminal.app** (`TERM_PROGRAM=Apple_Terminal`): Uses `osascript` to open a new tab in the front window and `cd` to `<dir>`.
- **iTerm2** (`TERM_PROGRAM=iTerm.app`): Uses `osascript` to create a new tab in the current window with `cd` to `<dir>`.
- **WezTerm** (`TERM_PROGRAM=WezTerm`): Uses `wezterm cli spawn --cwd <dir>`.
- **kitty** (`TERM_PROGRAM=kitty`): Uses `kitty @ launch --type=tab --cwd <dir>`.
- **Fallback**: `open -a Terminal <dir>` (macOS) or prints a clear message in the status bar.

---

## 7. CLI Interface & Versioning

- `lazyproject [PATH]` — Launch TUI in `PATH` (defaults to `.`)
- `lazyproject --version` / `-V` — Print version (e.g. `0.1.0+abc1234` for local builds, `0.1.0` for tagged releases)
- `lazyproject --help` / `-h` — Print usage

---

## 8. Quality, Testing, CI & Release Pipeline

- **Automated Checks**:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-targets`
  - `cargo audit` / `cargo deny check` (enforced in CI; run locally if installed)
- **Required Test Coverage**:
  - Unit tests for: URL credential sanitization, provider detection (GitHub/GitLab/Bitbucket/SSH/HTTPS), mise/toolchain parsing, sorting by activity, non-git collapsing.
  - Integration tests using temporary git repos (`tempfile`): dirty/untracked/staged detection, ahead/behind detection against a local bare remote, symlink/permission resilience, and verifying read-only guarantees (no files/refs modified during scan).
- **Git Workflow**:
  - Feature branches -> PR -> CI must pass -> Rebase merge into `main` (no merge commits).
  - Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, etc.).
- **Local Release & Install (`scripts/local-release.sh` / `make install-local`)**:
  - Fails immediately if working tree is dirty (`git status --porcelain` non-empty).
  - Runs `fmt`, `clippy`, `test` (and `cargo audit` / `cargo deny` if available).
  - Builds `--release` with `LAZYPROJECT_BUILD_SHA=<short-sha>` embedded into `--version`.
  - Installs binary to `~/.cargo/bin/lazyproject`.
- **Official Tag Releases**:
  - Pushing a `v*` tag (e.g., `v0.1.0`) triggers GitHub Actions to run full checks, build release binaries, and publish a GitHub Release with changelog notes.
