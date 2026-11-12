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

- **Browser extension** (TypeScript, Manifest V3): Chromium-based browsers (Chrome, Edge, Brave, Opera) and Firefox. Safari has no bookmarks API and is not planned.
- **Host** (`boom-host`, Rust): a native messaging host and CLI in one binary, `boom`. The browser starts it with the extension origin. Any other call of the form `boom cli <command>` runs the CLI.
- **Protocol:** each native messaging call is stateless. The extension sends one JSON request and reads one JSON response. Requests name an `action`, such as `ping`, `config_show`, `config_set` or `gitrepo_create`.
- **Config:** `~/.config/boom/config.toml`, on all platforms. The host owns this file, and the extension changes it only through the host.

## CLI

```bash
boom cli ping
boom cli config show
boom cli config set user-name "John Doe"
boom cli config set user-email "john@example.com"
boom cli config set repo-path /path/to/repo
boom cli gitrepo create /path/to/new/repo --remote git@github.com:you/bookmarks.git
boom cli gitrepo set-remote git@github.com:you/bookmarks.git
boom cli sync
```

`gitrepo create` needs `user-name` and `user-email` set first. It creates a local repo on branch `main` with a root `.metadata` file and an initial commit. It refuses a directory that is not empty. `--remote` sets `origin` at creation. `gitrepo set-remote` sets or adds `origin` in the configured repo.

`sync` commits local changes in the repo, then fetches, merges and pushes `origin` on the current branch. It needs `user-name`, `user-email` and `repo-path` set, and an `origin` remote configured in the repo.

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

CI runs format, clippy and tests on Ubuntu, macOS and Windows, and the extension tests on Ubuntu (`.github/workflows/test.yml`).

### Browser extension

Requirements: Node.js 26.

```bash
cd boom-extension
npm ci
npm test
```

`npm run build` writes two unpacked extensions:

- `boom-extension/dist/` for Chromium browsers (Chrome, Edge, Brave, Opera).
- `boom-extension/dist-firefox/` for Firefox.

#### Chromium

Load `boom-extension/dist` as an unpacked extension (developer mode on the extensions page). The browser shows an ID for it. Copy `native-host/chrome/com.boom.host.json`, replace `/ABSOLUTE/PATH/TO/boom` with the path of the `boom` binary and `<EXTENSION_ID>` with the ID, then place it in the browser's native messaging folder. On macOS for Chrome:

```bash
cp native-host/chrome/com.boom.host.json ~/Library/Application\ Support/Google/Chrome/NativeMessagingHosts/
```

#### Firefox

Firefox reads its own host manifest, from `native-host/firefox/`. The extension ID is fixed in `boom-extension/src/manifest.firefox.json` (`browser_specific_settings.gecko.id`), so no ID needs replacing. Copy `native-host/firefox/com.boom.host.json`, set `path` to the absolute path of the `boom` binary, and place it in:

- macOS: `~/Library/Application Support/Mozilla/NativeMessagingHosts/com.boom.host.json`
- Linux: `~/.mozilla/native-messaging-hosts/com.boom.host.json`
- Windows: a registry key `HKCU\Software\Mozilla\NativeMessagingHosts\com.boom.host` whose default value is the full path of the JSON file.

To load the extension in Firefox, open `about:debugging`, choose "This Firefox", then "Load Temporary Add-on" and pick `boom-extension/dist-firefox/manifest.json`.

## Repository layout

| Path | Content |
| --- | --- |
| `boom-host/` | Native messaging host and CLI (`boom` binary) |
| `boom-bookmarks/` | Sync library: bookmark model, planner, git reads, writes and sync |
| `boom-extension/` | Browser extension (TypeScript, Manifest V3, Chromium and Firefox builds) |
| `native-host/` | Native messaging host manifests, one folder per browser family |
| `.github/workflows/` | CI |

## Roadmap

- [x] Local git repo creation and config management
- [ ] Bookmark sync library: git reads and writes, state calculation
- [ ] Browser extension for Chromium-based browsers (scaffold and host ping done)
- [ ] Periodic pull from the extension
- [ ] Firefox support
- [ ] GitHub repo creation
- [ ] Windows and Linux packaging

## License

MIT. Copyright (c) 2026 Alex Ellwein.
