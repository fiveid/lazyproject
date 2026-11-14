# lazyproject Roadmap

To ship a fast, reliable, and usable tool quickly, **v0.1.0 is strictly scoped**. Ideas outside v0.1.0 are explicitly deferred to later releases.

## v0.1.0 — Core Read-Only Overview (Current Milestone)
- [x] Project spec, roadmap, `AGENTS.md`, `README.md`, `CHANGELOG.md`
- [x] Rust CLI & TUI (`ratatui` + `crossterm`)
- [x] Tabular project list with aligned columns (`Host | Project | Branch | Sync | Tree | Size | Age`)
- [x] Depth-1 workspace scanner with non-git directories hidden by default and toggled globally with `n`
- [x] Activity-based sorting (most recently changed at top, using `HEAD` commit + dirty file mtimes)
- [x] Git status & tracking inspection (branch, remote host GH/GL/BB, ahead/behind, dirty counts `+S ~M ?U !C`, last 3 commits)
- [x] Toolchain & package manager inspection (`mise.toml` / `.mise.toml` / `.tool-versions`, `package.json` + lockfiles, `Cargo.toml`, `pyproject.toml`, `go.mod`)
- [x] Background non-blocking disk usage + safely reclaimable dependency/build size (`node_modules`, `target`, `.venv`, etc.)
- [x] Manual remote fetch (`f` single, `F` all) with live per-row status indicator — zero network on startup
- [x] Open project in new terminal tab on macOS (Ghostty, Terminal.app, iTerm2, WezTerm, kitty)
- [x] Inspector focus/scroll (`Enter` / `Tab`), filter by name (`/`), help overlay (`?`), refresh (`r`)
- [x] Unit & integration tests (URL credential sanitization, provider detection, read-only scanning, disk usage)
- [x] GitHub Actions CI (fmt, clippy, test, cargo-deny) + `v*` tag release workflow
- [x] Gated local release & install script (`scripts/local-release.sh` + `Makefile`)

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
