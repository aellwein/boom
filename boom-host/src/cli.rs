//! Command-line interface, invoked as `boom cli <command>`.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use crate::config::{self, ConfigKey};
use crate::protocol::{self, Action, Request};

#[derive(Debug, Parser)]
#[command(name = "boom", version, about = "Git-backed bookmark sync")]
pub struct Cli {
    #[command(subcommand)]
    pub command: TopCommand,
}

#[derive(Debug, Subcommand)]
pub enum TopCommand {
    /// Command-line interface for managing boom.
    Cli {
        #[command(subcommand)]
        command: CliCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Health check of the host.
    Ping,
    /// Inspect or change the config.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Manage the bookmark git repo.
    Gitrepo {
        #[command(subcommand)]
        command: GitrepoCommand,
    },
    /// Commit local changes, then pull and push the bookmark repo.
    Sync,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the current config as TOML.
    Show,
    /// Set one config key, e.g. `config set user-name "Alex"`.
    Set {
        /// Key to change.
        key: ConfigKey,
        /// New value.
        value: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GitrepoCommand {
    /// Create a local repo at PATH. Requires user-name and user-email.
    Create {
        /// Directory for the new repo. Must be empty or missing.
        path: PathBuf,
    },
}

/// Runs a CLI command and prints the result. Returns the process exit code.
pub fn run(cli: Cli) -> i32 {
    let config_path = match config::default_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    match cli.command {
        TopCommand::Cli { command } => match command {
            CliCommand::Ping => dispatch(Action::Ping, &config_path),
            CliCommand::Config {
                command: ConfigCommand::Show,
            } => show_config(&config_path),
            CliCommand::Config {
                command: ConfigCommand::Set { key, value },
            } => dispatch(Action::ConfigSet { key, value }, &config_path),
            CliCommand::Gitrepo {
                command: GitrepoCommand::Create { path },
            } => dispatch(Action::GitrepoCreate { path }, &config_path),
            CliCommand::Sync => dispatch(Action::Sync, &config_path),
        },
    }
}

/// Prints the config file as TOML, the same format it is stored in.
fn show_config(config_path: &Path) -> i32 {
    match config_as_toml(config_path) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

fn config_as_toml(config_path: &Path) -> Result<String, String> {
    let config = config::load(config_path).map_err(|err| err.to_string())?;
    toml::to_string(&config).map_err(|err| err.to_string())
}

fn dispatch(action: Action, config_path: &Path) -> i32 {
    let response = protocol::handle(
        Request {
            id: "cli".into(),
            action,
        },
        config_path,
    );
    match serde_json::to_string_pretty(&response) {
        Ok(text) => println!("{text}"),
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    }
    if response.ok {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_config_show() {
        let cli = Cli::try_parse_from(["boom", "cli", "config", "show"]).unwrap();
        assert!(matches!(
            cli.command,
            TopCommand::Cli {
                command: CliCommand::Config {
                    command: ConfigCommand::Show
                }
            }
        ));
    }

    #[test]
    fn parses_config_set() {
        let cli = Cli::try_parse_from(["boom", "cli", "config", "set", "user-name", "Alex"]);
        assert!(cli.is_ok(), "{:?}", cli.err());
    }

    #[test]
    fn parses_gitrepo_create() {
        let cli = Cli::try_parse_from(["boom", "cli", "gitrepo", "create", "/tmp/bookmarks"]);
        assert!(cli.is_ok(), "{:?}", cli.err());
    }

    #[test]
    fn parses_sync() {
        let cli = Cli::try_parse_from(["boom", "cli", "sync"]);
        assert!(cli.is_ok(), "{:?}", cli.err());
    }

    #[test]
    fn config_show_prints_stored_config_as_toml() {
        let dir = std::env::temp_dir().join(format!("boom-cli-show-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");
        let config = config::Config {
            user_name: Some("Alex".into()),
            ..Default::default()
        };
        config::save(&path, &config).unwrap();

        let text = config_as_toml(&path).unwrap();
        assert!(!text.trim_start().starts_with('{'), "not TOML: {text}");
        let parsed: config::Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
