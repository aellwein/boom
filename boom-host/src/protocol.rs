//! Request and response types exchanged with the browser extension.
//!
//! Each native messaging call is stateless: the extension sends one
//! [`Request`] and reads one [`Response`]. The host reads the config and
//! the repo from disk for every request.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{self, ConfigKey};
use crate::gitrepo;

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
    GitrepoCreate { path: PathBuf },
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
        Action::Ping => Ok(json!({ "version": env!("CARGO_PKG_VERSION") })),
        Action::ConfigShow => {
            let config = config::load(config_path).map_err(|err| err.to_string())?;
            serde_json::to_value(config).map_err(|err| err.to_string())
        }
        Action::ConfigSet { key, value } => {
            let mut config = config::load(config_path).map_err(|err| err.to_string())?;
            config.set(key, value.clone());
            config::save(config_path, &config).map_err(|err| err.to_string())?;
            Ok(json!({ "key": key, "value": value }))
        }
        Action::GitrepoCreate { path } => {
            let mut config = config::load(config_path).map_err(|err| err.to_string())?;
            let (name, email) = match (&config.user_name, &config.user_email) {
                (Some(name), Some(email)) => (name.clone(), email.clone()),
                _ => return Err("set user_name and user_email before creating a repo".to_string()),
            };
            gitrepo::create(&path, &name, &email)?;
            config.repo_path = Some(path.clone());
            config::save(config_path, &config).map_err(|err| err.to_string())?;
            Ok(json!({ "path": path }))
        }
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
            r#"{"id":"2","action":"config_set","params":{"key":"user_name","value":"Alex"}}"#,
        )
        .unwrap();
        assert_eq!(
            request.action,
            Action::ConfigSet {
                key: ConfigKey::UserName,
                value: "Alex".into()
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
                value: "alex@example.com".into(),
            }),
            &path,
        );
        assert!(set.ok);

        let show = handle(request(Action::ConfigShow), &path);
        assert_eq!(show.result.unwrap()["user_email"], "alex@example.com");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gitrepo_create_requires_identity() {
        let dir = temp_path("identity");
        let response = handle(
            request(Action::GitrepoCreate {
                path: dir.join("bookmarks"),
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
            (ConfigKey::UserName, "Alex"),
            (ConfigKey::UserEmail, "alex@example.com"),
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

        let created = handle(request(Action::GitrepoCreate { path: repo.clone() }), &path);
        assert!(created.ok, "{:?}", created.error);
        assert!(repo.join(".git").exists());
        assert_eq!(config::load(&path).unwrap().repo_path, Some(repo.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
