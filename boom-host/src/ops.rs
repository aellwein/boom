//! Operations on the host's config, bookmark repo and sync.
//!
//! Both front ends call these functions: the native messaging protocol
//! (JSON, for the browser) and the CLI (text, for people). Errors are plain
//! messages, so either front end can show them as they are.

use std::path::Path;

use boom_bookmarks::git::{self, Integration};

use crate::config::{self, Config, ConfigKey};
use crate::gitrepo;

/// Version of the host, as set in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// Creates a bookmark repo at `path` and records it as `repo_path`.
pub fn gitrepo_create(config_path: &Path, path: &Path) -> Result<(), String> {
    let mut config = config_show(config_path)?;
    let (name, email) = identity(&config)
        .map_err(|_| "set user_name and user_email before creating a repo".to_string())?;
    gitrepo::create(path, name, email)?;
    config.repo_path = Some(path.to_path_buf());
    config::save(config_path, &config).map_err(|err| err.to_string())
}

/// Commits local changes in the bookmark repo, then fetches, merges and pushes `origin`.
///
/// The local commit happens first. If a later step fails, the commit stays
/// and the next sync retries the integration.
pub fn sync(config_path: &Path) -> Result<SyncReport, String> {
    let config = config_show(config_path)?;
    let (name, email) =
        identity(&config).map_err(|_| "set user_name and user_email before syncing".to_string())?;
    let repo_path = config
        .repo_path
        .as_deref()
        .ok_or_else(|| "set repo_path before syncing".to_string())?;

    let committed = git::commit_all(repo_path, SYNC_COMMIT_MESSAGE, name, email)
        .map_err(|err| err.to_string())?
        .is_some();
    let integration = git::sync(repo_path, name, email).map_err(|err| err.to_string())?;
    Ok(SyncReport {
        committed,
        integration,
    })
}

fn identity(config: &Config) -> Result<(&str, &str), ()> {
    match (&config.user_name, &config.user_email) {
        (Some(name), Some(email)) => Ok((name.as_str(), email.as_str())),
        _ => Err(()),
    }
}
