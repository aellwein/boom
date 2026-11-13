//! Operations on the host's config, bookmark repo and sync.
//!
//! Both front ends call these functions: the native messaging protocol
//! (JSON, for the browser) and the CLI (text, for people). Errors are plain
//! messages, so either front end can show them as they are.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use boom_bookmarks::git::{self, Integration};
use boom_bookmarks::{Change, Entry, Node};
use serde::Serialize;

use crate::config::{self, Config, ConfigKey};
use crate::gitrepo;

/// Version of the host, as set in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Commit message for bookmark changes made in the browser.
const BROWSER_COMMIT_MESSAGE: &str = "browser: update bookmarks";

/// Commit message for local changes that `sync` records before integrating.
const SYNC_COMMIT_MESSAGE: &str = "sync: local changes";

/// What a sync did. `committed` is true when local changes were recorded first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub committed: bool,
    pub integration: Integration,
}

/// Loads the config. A missing file yields the defaults.
pub fn config_show(config_path: &Path) -> Result<Config, String> {
    config::load(config_path).map_err(|err| err.to_string())
}

/// Sets one config key and saves the file.
pub fn config_set(config_path: &Path, key: ConfigKey, value: String) -> Result<(), String> {
    let mut config = config_show(config_path)?;
    config.set(key, value);
    config::save(config_path, &config).map_err(|err| err.to_string())
}

/// Creates a bookmark repo at `path`, records it as `repo_path`, and sets
/// `origin` to `remote` when one is given.
pub fn gitrepo_create(config_path: &Path, path: &Path, remote: Option<&str>) -> Result<(), String> {
    let mut config = config_show(config_path)?;
    let (name, email) = identity(&config)
        .map_err(|_| "set user_name and user_email before creating a repo".to_string())?;
    gitrepo::create(path, name, email)?;
    if let Some(url) = remote {
        git::set_origin(path, url).map_err(|err| err.to_string())?;
    }
    config.repo_path = Some(path.to_path_buf());
    config::save(config_path, &config).map_err(|err| err.to_string())
}

/// One entry of the bookmark repo: a folder or a bookmark, with its path
/// relative to the repo root. Entries come sorted by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoEntry {
    pub path: String,
    pub kind: &'static str,
    pub title: String,
    pub url: Option<String>,
}

/// The top-level folders a browser tree may be written to, and their order.
pub const ROOT_FOLDER_NAMES: [&str; 3] = ["bookmarks-bar", "other", "mobile"];

/// Commit id of the configured repo's HEAD.
pub fn repo_head(config_path: &Path) -> Result<String, String> {
    let config = config_show(config_path)?;
    let repo_path = repo_path(&config)?;
    git::head_id(repo_path).map_err(|err| err.to_string())
}

/// Makes the repo's bookmarks match `tree`, then commits and pushes.
///
/// `tree` holds the three browser roots as top-level folders named
/// `bookmarks-bar`, `other` and `mobile`. The write is refused unless HEAD
/// is `expected_head`, so changes made outside this browser are never
/// overwritten. The push is not preceded by a merge: if origin has moved,
/// the push fails and nothing is lost. Returns the new HEAD.
pub fn write_browser_tree(
    config_path: &Path,
    tree: &[boom_bookmarks::Node],
    expected_head: &str,
) -> Result<String, String> {
    let config = config_show(config_path)?;
    let (name, email) =
        identity(&config).map_err(|_| "set user_name and user_email before writing".to_string())?;
    let repo_path = repo_path(&config)?;
    require_origin(repo_path)?;

    for node in tree {
        let known = matches!(node, Node::Folder { title, .. } if ROOT_FOLDER_NAMES.contains(&title.as_str()));
        if !known {
            return Err(
                "browser tree must hold only the top-level folders bookmarks-bar, other and mobile"
                    .to_string(),
            );
        }
    }
    if !git::is_clean(repo_path).map_err(|err| err.to_string())? {
        return Err("the bookmark repo has uncommitted changes; run: boom cli sync".to_string());
    }
    let head = git::head_id(repo_path).map_err(|err| err.to_string())?;
    if head != expected_head {
        return Err("the repo changed outside this browser, so nothing was written".to_string());
    }

    let current = boom_bookmarks::read_tree(repo_path).map_err(|err| err.to_string())?;
    let changes = boom_bookmarks::plan(&current, tree);
    boom_bookmarks::apply(repo_path, &changes).map_err(|err| err.to_string())?;
    git::commit_all(repo_path, BROWSER_COMMIT_MESSAGE, name, email)
        .map_err(|err| err.to_string())?;
    if let Err(err) = git::push(repo_path) {
        // Undo this write, so HEAD is again the commit the browser last saw.
        // The next write rebuilds the same change from the browser.
        git::reset_hard(repo_path, expected_head).map_err(|reset| reset.to_string())?;
        return Err(format!("push failed, the write was undone: {err}"));
    }
    git::head_id(repo_path).map_err(|err| err.to_string())
}

/// One step that brings the browser from the commit `since` to the working
/// tree. Paths are repo paths with `/`. `index` is the position among the
/// entry's siblings in the new tree, and `parent` is None at the top level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RepoChange {
    RemoveBookmark {
        path: String,
    },
    RemoveFolder {
        path: String,
    },
    CreateFolder {
        path: String,
        parent: Option<String>,
        index: usize,
        title: String,
    },
    WriteBookmark {
        path: String,
        parent: Option<String>,
        index: usize,
        title: String,
        url: String,
    },
}

/// The changes that turn the repo at commit `since` into the working tree.
pub fn repo_changes_since(config_path: &Path, since: &str) -> Result<Vec<RepoChange>, String> {
    let config = config_show(config_path)?;
    let repo_path = repo_path(&config)?;
    let old =
        boom_bookmarks::read_tree_at_commit(repo_path, since).map_err(|err| err.to_string())?;
    let new = boom_bookmarks::read_tree(repo_path).map_err(|err| err.to_string())?;
    let new_flat = boom_bookmarks::flatten(&new);

    let mut changes = Vec::new();
    for change in boom_bookmarks::plan(&old, &new) {
        let entry = match change {
            Change::RemoveBookmark(path) => RepoChange::RemoveBookmark {
                path: repo_relative(&path),
            },
            Change::RemoveFolder(path) => RepoChange::RemoveFolder {
                path: repo_relative(&path),
            },
            Change::CreateFolder(path) => RepoChange::CreateFolder {
                index: sibling_index(&new_flat, &path),
                parent: parent_of(&path),
                title: title_from_path(&path),
                path: repo_relative(&path),
            },
            Change::WriteBookmark { path, bookmark } => RepoChange::WriteBookmark {
                index: sibling_index(&new_flat, &path),
                parent: parent_of(&path),
                title: bookmark.title,
                url: bookmark.url,
                path: repo_relative(&path),
            },
        };
        changes.push(entry);
    }
    Ok(changes)
}

/// Repo path of the parent folder, or None for the top level.
fn parent_of(path: &Path) -> Option<String> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(repo_relative)
}

/// Position of `path` among its siblings in `flat`, in repo order.
fn sibling_index(flat: &BTreeMap<PathBuf, boom_bookmarks::Entry>, path: &Path) -> usize {
    flat.keys()
        .filter(|other| other.parent() == path.parent() && other.as_path() < path)
        .count()
}

/// Lists every folder and bookmark of the configured repo.
pub fn repo_entries(config_path: &Path) -> Result<Vec<RepoEntry>, String> {
    let config = config_show(config_path)?;
    let repo_path = repo_path(&config)?;
    let nodes = boom_bookmarks::read_tree(repo_path).map_err(|err| err.to_string())?;
    Ok(boom_bookmarks::flatten(&nodes)
        .into_iter()
        .map(|(path, entry)| match entry {
            Entry::Folder => RepoEntry {
                path: repo_relative(&path),
                kind: "folder",
                title: title_from_path(&path),
                url: None,
            },
            Entry::Bookmark(bookmark) => RepoEntry {
                path: repo_relative(&path),
                kind: "bookmark",
                title: bookmark.title,
                url: Some(bookmark.url),
            },
        })
        .collect())
}

/// A repo path with `/` between components, on every platform. The
/// extension splits paths on `/`, so Windows separators must not leak out.
fn repo_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Folder title from its path: the last component without the position prefix.
fn title_from_path(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.split_once('_') {
        Some((digits, rest)) if digits.parse::<u64>().is_ok() => rest.to_string(),
        _ => name,
    }
}

/// Where the bookmark repo is and what its `origin` points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoStatus {
    pub repo_path: Option<PathBuf>,
    pub origin_url: Option<String>,
}

/// Reads the configured repo path and its `origin` URL.
pub fn gitrepo_status(config_path: &Path) -> Result<RepoStatus, String> {
    let config = config_show(config_path)?;
    let Some(path) = config.repo_path else {
        return Ok(RepoStatus {
            repo_path: None,
            origin_url: None,
        });
    };
    let origin_url = git::origin_url(&path).map_err(|err| err.to_string())?;
    Ok(RepoStatus {
        repo_path: Some(path),
        origin_url,
    })
}

/// Sets the `origin` URL of the bookmark repo, adding the remote if needed.
pub fn gitrepo_set_remote(config_path: &Path, url: &str) -> Result<(), String> {
    let config = config_show(config_path)?;
    let repo_path = repo_path(&config)?;
    git::set_origin(repo_path, url).map_err(|err| err.to_string())
}

/// Commits local changes in the bookmark repo, then fetches, merges and pushes `origin`.
///
/// The local commit happens first. If a later step fails, the commit stays
/// and the next sync retries the integration.
pub fn sync(config_path: &Path) -> Result<SyncReport, String> {
    let config = config_show(config_path)?;
    let (name, email) =
        identity(&config).map_err(|_| "set user_name and user_email before syncing".to_string())?;
    let repo_path = repo_path(&config)?;
    require_origin(repo_path)?;

    let committed = git::commit_all(repo_path, SYNC_COMMIT_MESSAGE, name, email)
        .map_err(|err| err.to_string())?
        .is_some();
    let integration = git::sync(repo_path, name, email).map_err(|err| err.to_string())?;
    Ok(SyncReport {
        committed,
        integration,
    })
}

/// Fetches and merges origin without committing or pushing.
///
/// Refuses to run while the repo has uncommitted changes, because merging
/// rewrites the working tree. Run `sync` first to record them.
pub fn pull(config_path: &Path) -> Result<Integration, String> {
    let config = config_show(config_path)?;
    let (name, email) =
        identity(&config).map_err(|_| "set user_name and user_email before pulling".to_string())?;
    let repo_path = repo_path(&config)?;
    require_origin(repo_path)?;
    if !git::is_clean(repo_path).map_err(|err| err.to_string())? {
        return Err("the bookmark repo has uncommitted changes; run: boom cli sync".to_string());
    }
    git::integrate(repo_path, name, email).map_err(|err| err.to_string())
}

fn require_origin(repo_path: &Path) -> Result<(), String> {
    if git::has_origin(repo_path).map_err(|err| err.to_string())? {
        Ok(())
    } else {
        Err(format!(
            "{} has no 'origin' remote; run: boom cli gitrepo set-remote <url>",
            repo_path.display()
        ))
    }
}

fn repo_path(config: &Config) -> Result<&Path, String> {
    config
        .repo_path
        .as_deref()
        .ok_or_else(|| "set repo_path first: boom cli gitrepo create <path>".to_string())
}

fn identity(config: &Config) -> Result<(&str, &str), ()> {
    match (&config.user_name, &config.user_email) {
        (Some(name), Some(email)) => Ok((name.as_str(), email.as_str())),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process;

    use boom_bookmarks::git;
    use git2::Repository;
    use git2::RepositoryInitOptions;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("boom-ops-{name}-{}", process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_identity(config_path: &Path) {
        config_set(config_path, ConfigKey::UserName, "John".into()).unwrap();
        config_set(config_path, ConfigKey::UserEmail, "john@example.com".into()).unwrap();
    }

    #[test]
    fn sync_without_origin_suggests_set_remote() {
        let dir = temp_dir("no_origin");
        let config_path = dir.join("config.toml");
        with_identity(&config_path);
        gitrepo_create(&config_path, &dir.join("bookmarks"), None).unwrap();

        let err = sync(&config_path).unwrap_err();
        assert!(err.contains("gitrepo set-remote"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_with_remote_then_sync_pushes() {
        let dir = temp_dir("with_remote");
        let config_path = dir.join("config.toml");
        let remote = dir.join("remote.git");
        with_identity(&config_path);
        Repository::init_opts(
            &remote,
            RepositoryInitOptions::new().bare(true).initial_head("main"),
        )
        .unwrap();

        gitrepo_create(&config_path, &dir.join("bookmarks"), remote.to_str()).unwrap();
        let report = sync(&config_path).unwrap();
        assert_eq!(report.integration, Integration::UpToDate);
        assert!(
            Repository::open(&remote)
                .unwrap()
                .find_reference("refs/heads/main")
                .is_ok()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_remote_needs_repo_path() {
        let dir = temp_dir("no_repo");
        let config_path = dir.join("config.toml");
        let err = gitrepo_set_remote(&config_path, "https://example.invalid/x.git").unwrap_err();
        assert!(err.contains("gitrepo create"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pull_requires_repo_path() {
        let dir = temp_dir("pull_no_repo");
        let config_path = dir.join("config.toml");
        with_identity(&config_path);
        let err = pull(&config_path).unwrap_err();
        assert!(err.contains("gitrepo create"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    fn repo_with_remote(dir: &Path) -> (PathBuf, PathBuf) {
        let config_path = dir.join("config.toml");
        let remote = dir.join("remote.git");
        with_identity(&config_path);
        Repository::init_opts(
            &remote,
            RepositoryInitOptions::new().bare(true).initial_head("main"),
        )
        .unwrap();
        gitrepo_create(&config_path, &dir.join("bookmarks"), remote.to_str()).unwrap();
        sync(&config_path).unwrap();
        (config_path, remote)
    }

    #[test]
    fn pull_after_sync_is_up_to_date() {
        let dir = temp_dir("pull_up_to_date");
        let (config_path, _remote) = repo_with_remote(&dir);
        assert_eq!(pull(&config_path).unwrap(), Integration::UpToDate);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pull_refuses_uncommitted_changes() {
        let dir = temp_dir("pull_dirty");
        let (config_path, _remote) = repo_with_remote(&dir);
        fs::write(
            dir.join("bookmarks").join("0010_Docs.toml"),
            "title = \"D\"\nurl = \"https://d.example\"\n",
        )
        .unwrap();
        let err = pull(&config_path).unwrap_err();
        assert!(err.contains("uncommitted"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pull_fast_forwards_a_new_remote_commit() {
        let dir = temp_dir("pull_ff");
        let (config_path, remote) = repo_with_remote(&dir);

        // A second clone pushes a change, as another machine would.
        let other = dir.join("other");
        Repository::clone(remote.to_str().unwrap(), &other).unwrap();
        fs::write(
            other.join("0010_Docs.toml"),
            "title = \"D\"\nurl = \"https://d.example\"\n",
        )
        .unwrap();
        git::commit_all(&other, "other: add docs", "Other", "other@example.invalid").unwrap();
        git::push(&other).unwrap();

        assert_eq!(pull(&config_path).unwrap(), Integration::FastForward);
        assert!(dir.join("bookmarks").join("0010_Docs.toml").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_entries_lists_folders_and_bookmarks_with_paths() {
        let dir = temp_dir("repo_entries");
        let config_path = dir.join("config.toml");
        with_identity(&config_path);
        let repo = dir.join("bookmarks");
        gitrepo_create(&config_path, &repo, None).unwrap();
        fs::create_dir_all(repo.join("0010_Work")).unwrap();
        fs::write(
            repo.join("0010_Work").join("0010_Docs.toml"),
            "title = \"Docs\"\nurl = \"https://docs.example\"\n",
        )
        .unwrap();

        let entries = repo_entries(&config_path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "0010_Work");
        assert_eq!(entries[0].kind, "folder");
        assert_eq!(entries[0].title, "Work");
        assert_eq!(entries[1].path, "0010_Work/0010_Docs.toml");
        assert_eq!(entries[1].kind, "bookmark");
        assert_eq!(entries[1].title, "Docs");
        assert_eq!(entries[1].url.as_deref(), Some("https://docs.example"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn bookmark_node(title: &str, url: &str) -> boom_bookmarks::Node {
        Node::Bookmark(boom_bookmarks::Bookmark {
            title: title.into(),
            url: url.into(),
        })
    }

    #[test]
    fn write_refuses_a_stale_head() {
        let dir = temp_dir("write_stale");
        let (config_path, _remote) = repo_with_remote(&dir);
        let tree = vec![Node::Folder {
            title: "bookmarks-bar".into(),
            children: vec![bookmark_node("Docs", "https://docs.example")],
        }];
        let err = write_browser_tree(
            &config_path,
            &tree,
            "0000000000000000000000000000000000000000",
        )
        .unwrap_err();
        assert!(err.contains("changed outside"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_commits_and_pushes_the_browser_tree() {
        let dir = temp_dir("write_push");
        let (config_path, remote) = repo_with_remote(&dir);
        let head = repo_head(&config_path).unwrap();
        let tree = vec![Node::Folder {
            title: "bookmarks-bar".into(),
            children: vec![bookmark_node("Docs", "https://docs.example")],
        }];

        let new_head = write_browser_tree(&config_path, &tree, &head).unwrap();
        assert_ne!(new_head, head);
        assert!(
            dir.join("bookmarks")
                .join("0010_bookmarks-bar")
                .join("0010_Docs.toml")
                .exists()
        );

        let pushed = Repository::open(&remote)
            .unwrap()
            .find_reference("refs/heads/main")
            .unwrap()
            .target()
            .unwrap()
            .to_string();
        assert_eq!(pushed, new_head);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_rejects_unknown_top_level_folders() {
        let dir = temp_dir("write_unknown");
        let (config_path, _remote) = repo_with_remote(&dir);
        let head = repo_head(&config_path).unwrap();
        let tree = vec![Node::Folder {
            title: "Random".into(),
            children: vec![],
        }];
        let err = write_browser_tree(&config_path, &tree, &head).unwrap_err();
        assert!(err.contains("bookmarks-bar"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_is_undone_when_the_push_fails() {
        let dir = temp_dir("write_undo");
        let (config_path, remote) = repo_with_remote(&dir);
        let head = repo_head(&config_path).unwrap();

        // Another clone pushes first, so this push will be rejected.
        let other = dir.join("other");
        Repository::clone(remote.to_str().unwrap(), &other).unwrap();
        fs::write(
            other.join("0010_Other.toml"),
            "title = \"O\"\nurl = \"https://o.example\"\n",
        )
        .unwrap();
        git::commit_all(&other, "other: add", "Other", "other@example.invalid").unwrap();
        git::push(&other).unwrap();

        let tree = vec![Node::Folder {
            title: "bookmarks-bar".into(),
            children: vec![bookmark_node("Docs", "https://docs.example")],
        }];
        let err = write_browser_tree(&config_path, &tree, &head).unwrap_err();
        assert!(err.contains("write was undone"), "{err}");
        assert_eq!(repo_head(&config_path).unwrap(), head);
        assert!(
            !dir.join("bookmarks")
                .join("0010_bookmarks-bar")
                .join("0010_Docs.toml")
                .exists()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changes_since_a_commit_describe_the_new_bookmarks() {
        let dir = temp_dir("changes_since");
        let (config_path, _remote) = repo_with_remote(&dir);
        let base = repo_head(&config_path).unwrap();

        let repo = dir.join("bookmarks");
        fs::create_dir_all(repo.join("0010_bookmarks-bar")).unwrap();
        fs::write(
            repo.join("0010_bookmarks-bar").join("0010_Docs.toml"),
            "title = \"Docs\"\nurl = \"https://docs.example\"\n",
        )
        .unwrap();
        git::commit_all(&repo, "test: add docs", "John Doe", "john@example.com").unwrap();

        let changes = repo_changes_since(&config_path, &base).unwrap();
        // The root folder is new in git, so it is listed too. The browser
        // side skips it, because the root is already mapped.
        assert_eq!(
            changes,
            vec![
                RepoChange::CreateFolder {
                    path: "0010_bookmarks-bar".into(),
                    parent: None,
                    index: 0,
                    title: "bookmarks-bar".into(),
                },
                RepoChange::WriteBookmark {
                    path: "0010_bookmarks-bar/0010_Docs.toml".into(),
                    parent: Some("0010_bookmarks-bar".into()),
                    index: 0,
                    title: "Docs".into(),
                    url: "https://docs.example".into(),
                },
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
