# boom

A bookmark manager that keeps browser bookmarks in sync through a git repository. Each machine with a browser can share the same bookmarks.

> **Status: early development.** The host and CLI skeleton works. The browser extension and bookmark sync are not built yet.

## How it works

Planned design. Bookmarks will be stored as plain files in a git repository:

- a bookmark folder maps to a directory
- a bookmark maps to a file inside its folder
- the root `.metadata` file holds shared settings and is not a bookmark

Changes will flow both ways. A bookmark created in the browser is committed and pushed. A commit pulled from git creates or removes the matching bookmark in the browser. Conflicts will be resolved with last writer wins by default.

## Architecture

- **Browser extension** (TypeScript, planned): Chromium-based browsers first (Chrome, Edge, Brave, Opera), then Firefox. Safari has no bookmarks API and is not planned.
- **Host** (`boom-host`, Rust): a native messaging host and CLI in one binary, `boom`. The browser starts it with the extension origin. Any other call of the form `boom cli <command>` runs the CLI.
- **Protocol:** each native messaging call is stateless. The extension sends one JSON request and reads one JSON response. Requests name an `action`, such as `ping`, `config_show`, `config_set` or `gitrepo_create`.
- **Config:** `~/.config/boom/config.toml`, on all platforms. The host owns this file, and the extension changes it only through the host.

## CLI

```bash
boom cli ping
boom cli config show
boom cli config set user-name "John Doe"
boom cli config set user-email "alex@example.com"
boom cli config set repo-path /path/to/repo
boom cli gitrepo create /path/to/new/repo
```

`gitrepo create` needs `user-name` and `user-email` set first. It creates a local repo on branch `main` with a root `.metadata` file and an initial commit. It refuses a directory that is not empty.

## Platforms

Planned: macOS, Linux and Windows.

## Development

Requirements: Rust (stable), and a C compiler for `git2`.

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

CI runs format, clippy and tests on Ubuntu, macOS and Windows (`.github/workflows/test.yml`).

## Repository layout

| Path | Content |
| --- | --- |
| `boom-host/` | Native messaging host and CLI (`boom` binary) |
| `boom-bookmarks/` | Sync library (placeholder) |
| `.github/workflows/` | CI |

## Roadmap

- Local git repo creation and config management (done)
- Bookmark sync library: git reads and writes, state calculation
- Browser extension for Chromium-based browsers
- Periodic pull from the extension
- Firefox support
- GitHub repo creation
- Windows and Linux packaging

## License

MIT. Copyright (c) 2026 Alex Ellwein.
