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
    if !git::has_origin(repo_path).map_err(|err| err.to_string())? {
        return Err(format!(
            "{} has no 'origin' remote; run: boom cli gitrepo set-remote <url>",
            repo_path.display()
        ));
    }

    let committed = git::commit_all(repo_path, SYNC_COMMIT_MESSAGE, name, email)
        .map_err(|err| err.to_string())?
        .is_some();
    let integration = git::sync(repo_path, name, email).map_err(|err| err.to_string())?;
    Ok(SyncReport {
        committed,
        integration,
    })
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
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("boom-ops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_identity(config_path: &Path) {
        config_set(config_path, ConfigKey::UserName, "Alex".into()).unwrap();
        config_set(config_path, ConfigKey::UserEmail, "alex@example.com".into()).unwrap();
    }

    #[test]
    fn sync_without_origin_suggests_set_remote() {
        let dir = temp_dir("no_origin");
        let config_path = dir.join("config.toml");
        with_identity(&config_path);
        gitrepo_create(&config_path, &dir.join("bookmarks"), None).unwrap();

        let err = sync(&config_path).unwrap_err();
        assert!(err.contains("gitrepo set-remote"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_with_remote_then_sync_pushes() {
        let dir = temp_dir("with_remote");
        let config_path = dir.join("config.toml");
        let remote = dir.join("remote.git");
        with_identity(&config_path);
        git2::Repository::init_opts(
            &remote,
            git2::RepositoryInitOptions::new()
                .bare(true)
                .initial_head("main"),
        )
        .unwrap();

        gitrepo_create(&config_path, &dir.join("bookmarks"), remote.to_str()).unwrap();
        let report = sync(&config_path).unwrap();
        assert_eq!(report.integration, Integration::UpToDate);
        assert!(git2::Repository::open(&remote)
            .unwrap()
            .find_reference("refs/heads/main")
            .is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_remote_needs_repo_path() {
        let dir = temp_dir("no_repo");
        let config_path = dir.join("config.toml");
        let err = gitrepo_set_remote(&config_path, "https://example.invalid/x.git").unwrap_err();
        assert!(err.contains("gitrepo create"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
