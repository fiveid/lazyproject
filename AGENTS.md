# AGENTS.md — Guidelines for AI & Human Contributors

## Project Goal
`lazyproject` is a fast, read-only-first Rust TUI for inspecting all projects in a workspace directory. See [docs/SPEC.md](docs/SPEC.md) for the full specification and [docs/ROADMAP.md](docs/ROADMAP.md) for scope boundaries.

## Non-Negotiable Invariants
1. **Read-Only First**: Never mutate user repositories during scanning, sorting, or inspection. `git fetch` is only allowed when explicitly triggered by the user (`f` / `F`). Never add destructive git operations (`reset`, `clean`, `checkout`, `push`, `commit`).
2. **No Secret Leakage**: Always strip credentials/userinfo from remote URLs before storing or rendering. Never read `.env` or credential files.
3. **No Shell Injection**: Always invoke subprocesses (`git`, `osascript`, `open`, `wezterm`, `kitty`) with explicit argument arrays (`Command::new(...).arg(...)`). Never use `sh -c`.
4. **No Scope Creep**: Stick strictly to the active milestone in [docs/ROADMAP.md](docs/ROADMAP.md).

## Architecture & Code Quality
- Keep modules small, focused, and testable:
  - `scanner`: directory discovery, classification (git vs non-git), activity timestamp calculation
  - `git`: read-only git inspection, URL sanitization, provider detection, explicit fetch
  - `toolchain`: `mise.toml` / `.tool-versions` / manifest / lockfile detection
  - `terminal`: macOS terminal tab/window launcher
  - `app`: TUI state, filtering, sorting, async/background task handling
  - `ui`: `ratatui` rendering
- All `clippy` warnings are errors (`-D warnings`).
- Keep `unsafe` out of the codebase (`#![forbid(unsafe_code)]`).

## Verification Commands
Run all of these before considering any change complete:
```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

## Git & Release Workflow
- **Branches**: Always work on feature branches (`feat/...`, `fix/...`, `chore/...`, `docs/...`).
- **Clean Commit History**:
  - Keep PRs to **one clean, self-contained commit** (or a minimal sequence of distinct logical commits).
  - When iterating on a PR (e.g. fixing review comments or CI failures), **amend** (`git commit --amend`) or **interactive-rebase/squash** locally and force-push with lease (`git push --force-with-lease`) rather than adding `"fix again"` commits.
  - Never create merge commits (`git merge`); always rebase onto `main` (`git rebase origin/main`) and use rebase-merge on GitHub.
- **Conventional Commits**:
  - Every commit subject MUST match `<type>[optional (scope)][!]: <imperative summary>` (enforced by `.githooks/commit-msg`).
  - Allowed types: `feat`, `fix`, `docs`, `chore`, `ci`, `test`, `refactor`.
  - Enable the local hook with `make setup-hooks`.
- **Local Install**: Use `make install-local` (or `./scripts/local-release.sh`), which gates installation on a clean working tree and passing checks, and embeds `<version>+<short-sha>`.
- **Official Releases**: Update `CHANGELOG.md` (move `[Unreleased]` under `## [X.Y.Z] - YYYY-MM-DD`), merge to `main`, tag `vX.Y.Z`, and push the tag to trigger the GitHub Release workflow.
