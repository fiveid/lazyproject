# lazyproject Roadmap

To ship a fast, reliable, and usable tool quickly, **v0.1.0 is strictly scoped**. Ideas outside v0.1.0 are explicitly deferred to later releases.

## v0.1.0 — Core Read-Only Overview (Current Milestone)
- [x] Project spec, roadmap, `AGENTS.md`, `README.md`, `CHANGELOG.md`
- [ ] Rust CLI & TUI (`ratatui` + `crossterm`)
- [ ] Depth-1 workspace scanner with non-git collapsing (`+ N projects not on Git`)
- [ ] Activity-based sorting (most recently changed at top)
- [ ] Git status & tracking inspection (branch, remote host GH/GL, ahead/behind, dirty counts, last 3 commits)
- [ ] Toolchain & package manager inspection (`mise.toml` / `.mise.toml` / `.tool-versions`, `package.json` + lockfiles, `Cargo.toml`, `pyproject.toml`, `go.mod`)
- [ ] Manual remote fetch (`f` single, `F` all) — zero network on startup
- [ ] Open project in new terminal tab/window on macOS (Ghostty, Terminal.app, iTerm2, WezTerm, kitty)
- [ ] Filter by name (`/`), help overlay (`?`), refresh (`r`)
- [ ] Comprehensive unit & integration tests (including read-only & URL sanitization tests)
- [ ] GitHub Actions CI (fmt, clippy, test, cargo-deny/audit) + `v*` tag release workflow
- [ ] Gated local release & install script (`scripts/local-release.sh` + `Makefile`)

## v0.2.0 — Polish & Productivity
- [ ] Configurable ignore patterns & root directory presets (`~/.config/lazyproject/config.toml`)
- [ ] Filter presets (e.g., "Only Dirty", "Only Ahead/Behind", "All")
- [ ] Open in `$EDITOR` (`e`)
- [ ] Stash count & worktree indicator
- [ ] Linux terminal tab support (GNOME Terminal, Konsole, Alacritty, Foot)

## v0.3.0 — Deep Inspection & Workflows
- [ ] Optional `git` diff preview (read-only) in inspector
- [ ] Dependency outdated / security summary (opt-in)
- [ ] Custom user actions / scripts
