# lazyproject

A fast, read-only Terminal UI (TUI) in the style of `lazydocker` and `lazygit` for getting an instant overview of all projects in a directory.

Run `lazyproject` in your projects folder to see:
- **What's uncommitted or unpushed** (so nothing important stays unbacked up)
- **Where remotes are ahead** (so you sync before making conflicting changes)
- **Remote host & branch status** (GitHub, GitLab, Bitbucket, local-only)
- **Required toolchains & package managers** (`mise.toml`, `.tool-versions`, `package.json` + lockfiles, `Cargo.toml`, `pyproject.toml`, `go.mod`)
- **Non-git folders** neatly collapsed at the bottom (`+ N projects not on Git`) until expanded

> **Read-only by design:** `lazyproject` never modifies your repositories or runs `git fetch` on startup. Remote fetches only happen when you explicitly press `f` (selected) or `F` (all).

## Installation

### Local Gated Release & Install
To build and install a local release to `~/.cargo/bin/lazyproject` (gated on a clean git tree, formatting, clippy, and all tests):

```sh
make install-local
```

This embeds the version and commit SHA (e.g. `0.1.0+a1b2c3d`) into `lazyproject --version`.

## Usage

```sh
# Inspect current directory
lazyproject

# Inspect a specific projects directory
lazyproject ~/Projects
```

### Keybindings

| Key | Action |
|---|---|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` / `G` | Jump to top / bottom |
| `Enter` / `Space` | Expand/collapse non-git projects row |
| `o` | Open project in a new terminal tab/window |
| `f` | Fetch remote for selected project |
| `F` | Fetch remotes for all git projects |
| `r` | Refresh / rescan |
| `/` | Filter projects by name |
| `?` | Toggle help |
| `q` | Quit |

## Documentation
- [Specification](docs/SPEC.md)
- [Roadmap](docs/ROADMAP.md)
- [Agent & Contributor Guide](AGENTS.md)
- [Changelog](CHANGELOG.md)
