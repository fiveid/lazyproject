# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-14

### Added
- Project specification (`docs/SPEC.md`), roadmap (`docs/ROADMAP.md`), `AGENTS.md`, `README.md`, and `CHANGELOG.md`.
- Read-only `lazyproject` Rust CLI and `ratatui` TUI with tabular project view (`Host | Project | Branch | Sync | Tree | Size | Age`).
- Parallel depth-1 workspace scanner with activity-based sorting and global `n` toggle for non-git directories.
- Git inspection (branch, upstream, ahead/behind, `+S ~M ?U !C` dirty counts, last 3 commits, GitHub/GitLab/Bitbucket detection, credential stripping) and manual remote fetch (`f` / `F`) with live per-row status indicators.
- Toolchain detection for `mise.toml`, `.mise.toml`, `.tool-versions`, Node.js, Rust, Python, and Go.
- Background non-blocking disk usage and safely reclaimable dependency/build directory calculation.
- macOS new terminal tab launcher for Ghostty (AppleScript), Terminal.app, iTerm2, WezTerm, and kitty.
- GitHub Actions CI (`fmt`, `clippy`, `test`, `cargo-deny`), `v*` tag release workflow, and gated local install script (`make install-local`).
