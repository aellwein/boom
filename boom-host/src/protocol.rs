//! JSON request and response types for the browser extension.
//!
//! Each native messaging call is stateless: the extension sends one
//! [`Request`] and reads one [`Response`]. The host reads the config and
//! the repo from disk for every request. The operations themselves live in
//! [`crate::ops`], which the CLI uses too.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use boom_bookmarks::git::Integration;

use crate::config::ConfigKey;
use crate::ops::{self, SyncReport};

/// One management or sync operation requested by the extension or the CLI.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "action", content = "params", rename_all = "snake_case")]
pub enum Action {
    /// Health check. Returns the host version.
    Ping,
    /// Returns the current config.
    ConfigShow,
    /// Sets one config key.
    ConfigSet { key: ConfigKey, value: String },
    /// Creates a local bookmark repo at `path` and records it in the config.
    /// `remote`, if given, is set as `origin`.
    GitrepoCreate {
        path: PathBuf,
        #[serde(default)]
        remote: Option<String>,
    },
    /// Sets the URL of `origin` in the configured bookmark repo.
    GitrepoSetRemote { url: String },
    /// Returns the configured repo path and its `origin` URL.
    GitrepoStatus,
    /// Lists every folder and bookmark in the configured repo.
    RepoEntries,
    /// Returns the commit id of the repo's HEAD.
    RepoHead,
    /// Lists the changes from commit `since` to the working tree.
    RepoChangesSince { since: String },
    /// Makes the repo match the browser tree, then commits and pushes.
    /// Refused unless HEAD is `expected_head`.
    WriteBrowserTree {
        tree: Vec<boom_bookmarks::Node>,
        expected_head: String,
    },
    /// Commits local changes in the bookmark repo, then pulls and pushes `origin`.
    Sync,
    /// Fetches and merges `origin` without committing or pushing.
    Pull,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Request {
    /// Caller-chosen id, echoed back in the response.
    pub id: String,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    fn success(id: String, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn failure(id: String, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

/// Runs one action against the config at `config_path`.
/// Errors become `ok: false` responses.
pub fn handle(request: Request, config_path: &Path) -> Response {
    let id = request.id;
    match run(request.action, config_path) {
        Ok(result) => Response::success(id, result),
        Err(error) => Response::failure(id, error),
    }
}

fn run(action: Action, config_path: &Path) -> Result<Value, String> {
    match action {
        Action::Ping => Ok(json!({ "version": ops::VERSION })),
        Action::ConfigShow => {
            serde_json::to_value(ops::config_show(config_path)?).map_err(|err| err.to_string())
        }
        Action::ConfigSet { key, value } => {
            ops::config_set(config_path, key, value.clone())?;
            Ok(json!({ "key": key, "value": value }))
        }
        Action::GitrepoCreate { path, remote } => {
            ops::gitrepo_create(config_path, &path, remote.as_deref())?;
            Ok(json!({ "path": path }))
        }
        Action::GitrepoStatus => {
            let status = ops::gitrepo_status(config_path)?;
            Ok(json!({
                "repo_path": status.repo_path,
                "origin_url": status.origin_url,
            }))
        }
        Action::RepoHead => Ok(json!({ "head": ops::repo_head(config_path)? })),
        Action::RepoChangesSince { since } => {
            serde_json::to_value(ops::repo_changes_since(config_path, &since)?)
                .map_err(|err| err.to_string())
        }
        Action::WriteBrowserTree {
            tree,
            expected_head,
        } => Ok(json!({
            "head": ops::write_browser_tree(config_path, &tree, &expected_head)?,
        })),
        Action::RepoEntries => {
            serde_json::to_value(ops::repo_entries(config_path)?).map_err(|err| err.to_string())
        }
        Action::GitrepoSetRemote { url } => {
            ops::gitrepo_set_remote(config_path, &url)?;
            Ok(json!({ "url": url }))
        }
        Action::Sync => Ok(sync_json(&ops::sync(config_path)?)),
        Action::Pull => {
            let (integration, conflicts) = integration_fields(ops::pull(config_path)?);
            Ok(json!({ "integration": integration, "conflicts": conflicts }))
        }
    }
}

fn sync_json(report: &SyncReport) -> Value {
    let (integration, conflicts) = integration_fields(report.integration);
    json!({
        "committed": report.committed,
        "integration": integration,
        "conflicts": conflicts,
    })
}

fn integration_fields(integration: Integration) -> (&'static str, usize) {
    match integration {
        Integration::UpToDate => ("up_to_date", 0),
        Integration::FastForward => ("fast_forward", 0),
        Integration::Merged { conflicts } => ("merged", conflicts),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("boom-proto-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(action: Action) -> Request {
        Request {
            id: "1".into(),
            action,
        }
    }

    #[test]
    fn parses_ping_request() {
        let request: Request = serde_json::from_str(r#"{"id":"1","action":"ping"}"#).unwrap();
        assert_eq!(request.id, "1");
        assert_eq!(request.action, Action::Ping);
    }

    #[test]
    fn parses_config_set_request() {
        let request: Request = serde_json::from_str(
            r#"{"id":"2","action":"config_set","params":{"key":"user_name","value":"John"}}"#,
        )
        .unwrap();
        assert_eq!(
            request.action,
            Action::ConfigSet {
                key: ConfigKey::UserName,
                value: "John".into()
            }
        );
    }

    #[test]
    fn unknown_action_fails_to_parse() {
        let parsed = serde_json::from_str::<Request>(r#"{"id":"1","action":"nope"}"#);
        assert!(parsed.is_err());
    }

    #[test]
    fn ping_returns_version() {
        let dir = temp_path("ping");
        let response = handle(request(Action::Ping), &dir.join("config.toml"));
        assert!(response.ok);
        assert_eq!(
            response.result.unwrap()["version"],
            env!("CARGO_PKG_VERSION")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_set_then_show() {
        let dir = temp_path("set");
        let path = dir.join("config.toml");

        let set = handle(
            request(Action::ConfigSet {
                key: ConfigKey::UserEmail,
                value: "john@example.com".into(),
            }),
            &path,
        );
        assert!(set.ok);

        let show = handle(request(Action::ConfigShow), &path);
        assert_eq!(show.result.unwrap()["user_email"], "john@example.com");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gitrepo_create_requires_identity() {
        let dir = temp_path("identity");
        let response = handle(
            request(Action::GitrepoCreate {
                path: dir.join("bookmarks"),
                remote: None,
            }),
            &dir.join("config.toml"),
        );
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("user_name"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gitrepo_create_records_repo_path() {
        let dir = temp_path("create");
        let path = dir.join("config.toml");
        let repo = dir.join("bookmarks");

        for (key, value) in [
            (ConfigKey::UserName, "John"),
            (ConfigKey::UserEmail, "john@example.com"),
        ] {
            let set = handle(
                request(Action::ConfigSet {
                    key,
                    value: value.into(),
                }),
                &path,
            );
            assert!(set.ok);
        }

        let created = handle(
            request(Action::GitrepoCreate {
                path: repo.clone(),
                remote: None,
            }),
            &path,
        );
        assert!(created.ok, "{:?}", created.error);
        assert!(repo.join(".git").exists());
        assert_eq!(
            crate::config::load(&path).unwrap().repo_path,
            Some(repo.clone())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn set_identity(path: &Path) {
        for (key, value) in [
            (ConfigKey::UserName, "John"),
            (ConfigKey::UserEmail, "john@example.com"),
        ] {
            let set = handle(
                request(Action::ConfigSet {
                    key,
                    value: value.into(),
                }),
                path,
            );
            assert!(set.ok);
        }
    }

    #[test]
    fn sync_requires_repo_path() {
        let dir = temp_path("sync_no_repo");
        let path = dir.join("config.toml");
        set_identity(&path);

        let response = handle(request(Action::Sync), &path);
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("repo_path"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_commits_and_pushes_to_origin() {
        let dir = temp_path("sync_push");
        let path = dir.join("config.toml");
        let repo = dir.join("bookmarks");
        let remote = dir.join("remote.git");
        set_identity(&path);

        let created = handle(
            request(Action::GitrepoCreate {
                path: repo.clone(),
                remote: None,
            }),
            &path,
        );
        assert!(created.ok, "{:?}", created.error);

        git2::Repository::init_opts(
            &remote,
            git2::RepositoryInitOptions::new()
                .bare(true)
                .initial_head("main"),
        )
        .unwrap();
        git2::Repository::open(&repo)
            .unwrap()
            .remote("origin", remote.to_str().unwrap())
            .unwrap();

        // First sync: nothing new to commit, remote has no branch yet, so it is created.
        let first = handle(request(Action::Sync), &path);
        assert!(first.ok, "{:?}", first.error);
        let first = first.result.unwrap();
        assert_eq!(first["committed"], false);
        assert_eq!(first["integration"], "up_to_date");

        // A new bookmark file is committed and pushed.
        std::fs::write(
            repo.join("0010_Docs.toml"),
            "title = \"Docs\"\nurl = \"https://docs.example\"\n",
        )
        .unwrap();
        let second = handle(request(Action::Sync), &path);
        assert!(second.ok, "{:?}", second.error);
        assert_eq!(second.result.unwrap()["committed"], true);

        let remote_repo = git2::Repository::open(&remote).unwrap();
        let remote_head = remote_repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        assert_eq!(remote_head.message().unwrap(), "sync: local changes");
        assert!(remote_head
            .tree()
            .unwrap()
            .get_name("0010_Docs.toml")
            .is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_gitrepo_create_without_remote() {
        let request: Request = serde_json::from_str(
            r#"{"id":"3","action":"gitrepo_create","params":{"path":"/tmp/x"}}"#,
        )
        .unwrap();
        assert_eq!(
            request.action,
            Action::GitrepoCreate {
                path: PathBuf::from("/tmp/x"),
                remote: None
            }
        );
    }

    #[test]
    fn gitrepo_set_remote_needs_repo_path() {
        let dir = temp_path("set_remote");
        let path = dir.join("config.toml");
        let response = handle(
            request(Action::GitrepoSetRemote {
                url: "https://example.invalid/x.git".into(),
            }),
            &path,
        );
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("gitrepo create"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gitrepo_status_reports_origin_of_configured_repo() {
        let dir = temp_path("status");
        let path = dir.join("config.toml");
        set_identity(&path);
        let repo = dir.join("bookmarks");
        let created = handle(
            request(Action::GitrepoCreate {
                path: repo.clone(),
                remote: Some("https://example.invalid/bm.git".into()),
            }),
            &path,
        );
        assert!(created.ok, "{:?}", created.error);

        let status = handle(request(Action::GitrepoStatus), &path);
        assert!(status.ok, "{:?}", status.error);
        let result = status.result.unwrap();
        assert_eq!(result["origin_url"], "https://example.invalid/bm.git");
        assert_eq!(result["repo_path"], repo.to_str().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gitrepo_status_without_repo_is_empty() {
        let dir = temp_path("status_empty");
        let path = dir.join("config.toml");
        let status = handle(request(Action::GitrepoStatus), &path);
        assert!(status.ok);
        let result = status.result.unwrap();
        assert!(result["repo_path"].is_null());
        assert!(result["origin_url"].is_null());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pull_action_reports_missing_repo() {
        let dir = temp_path("pull_action");
        let path = dir.join("config.toml");
        let response = handle(request(Action::Pull), &path);
        assert!(!response.ok);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
