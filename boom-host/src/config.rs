//! Machine-local config stored at `~/.config/boom/config.toml`.
//!
//! The host is the single source of truth for this file. The browser
//! extension reads and writes it only through the host.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// How to resolve a change that conflicts between machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// The newer commit wins.
    #[default]
    LastWriterWins,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Git author name for commits made by boom.
    pub user_name: Option<String>,
    /// Git author email for commits made by boom.
    pub user_email: Option<String>,
    /// Local path of the bookmark git repo.
    pub repo_path: Option<PathBuf>,
    /// Conflict resolution for bookmark changes.
    pub conflict_policy: ConflictPolicy,
    /// Seconds between periodic pulls, driven by the extension.
    pub poll_interval_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user_name: None,
            user_email: None,
            repo_path: None,
            conflict_policy: ConflictPolicy::default(),
            poll_interval_secs: 300,
        }
    }
}

/// Settings that can be changed with `config set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ConfigKey {
    UserName,
    UserEmail,
    RepoPath,
}

impl Config {
    /// Sets one key from a string value.
    pub fn set(&mut self, key: ConfigKey, value: String) {
        match key {
            ConfigKey::UserName => self.user_name = Some(value),
            ConfigKey::UserEmail => self.user_email = Some(value),
            ConfigKey::RepoPath => self.repo_path = Some(PathBuf::from(value)),
        }
    }
}

/// Returns `~/.config/boom/config.toml`. Used on all platforms.
pub fn default_path() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    Ok(home.join(".config").join("boom").join("config.toml"))
}

/// Loads the config from `path`. A missing file yields the defaults.
pub fn load(path: &Path) -> io::Result<Config> {
    match fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(err) => Err(err),
    }
}

/// Writes the config to `path`, creating parent directories as needed.
pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text =
        toml::to_string(config).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.conflict_policy, ConflictPolicy::LastWriterWins);
    }

    #[test]
    fn round_trips_toml() {
        let config = Config {
            user_name: Some("Alex Ellwein".into()),
            user_email: Some("alex@example.com".into()),
            repo_path: Some(PathBuf::from("/home/alex/bookmarks")),
            conflict_policy: ConflictPolicy::LastWriterWins,
            poll_interval_secs: 120,
        };
        let text = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn set_updates_the_matching_key() {
        let mut config = Config::default();
        config.set(ConfigKey::UserName, "Alex".into());
        config.set(ConfigKey::RepoPath, "/tmp/bookmarks".into());
        assert_eq!(config.user_name.as_deref(), Some("Alex"));
        assert_eq!(config.repo_path, Some(PathBuf::from("/tmp/bookmarks")));
        assert_eq!(config.user_email, None);
    }
}
