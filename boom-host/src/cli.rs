//! Command-line interface, invoked as `boom cli <command>`.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use boom_bookmarks::git::Integration;

use crate::config::{self, ConfigKey};
use crate::ops::{self, SyncReport};

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
    /// Set one config key, e.g. `config set user-name "John"`.
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
        /// URL of the remote, set as `origin`.
        #[arg(long)]
        remote: Option<String>,
    },
    /// Set the URL of `origin` in the configured repo, adding it if missing.
    SetRemote {
        /// URL of the remote, e.g. git@github.com:you/bookmarks.git.
        url: String,
    },
}

/// Runs a CLI command and prints the result for people. Returns the process exit code.
pub fn run(cli: Cli) -> i32 {
    let config_path = match config::default_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let result = match cli.command {
        TopCommand::Cli { command } => match command {
            CliCommand::Ping => {
                println!("boom {}", ops::VERSION);
                Ok(())
            }
            CliCommand::Config {
                command: ConfigCommand::Show,
            } => show_config(&config_path),
            CliCommand::Config {
                command: ConfigCommand::Set { key, value },
            } => ops::config_set(&config_path, key, value).map(|()| println!("config updated")),
            CliCommand::Gitrepo {
                command: GitrepoCommand::Create { path, remote },
            } => ops::gitrepo_create(&config_path, &path, remote.as_deref())
                .map(|()| println!("created bookmark repo at {}", path.display())),
            CliCommand::Gitrepo {
                command: GitrepoCommand::SetRemote { url },
            } => ops::gitrepo_set_remote(&config_path, &url)
                .map(|()| println!("origin set to {url}")),
            CliCommand::Sync => ops::sync(&config_path).map(|report| print_sync(&report)),
        },
    };

    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}

/// Prints the config file as TOML, the same format it is stored in.
fn show_config(config_path: &Path) -> Result<(), String> {
    print!("{}", config_as_toml(config_path)?);
    Ok(())
}

fn config_as_toml(config_path: &Path) -> Result<String, String> {
    let config = ops::config_show(config_path)?;
    toml::to_string(&config).map_err(|err| err.to_string())
}

fn print_sync(report: &SyncReport) {
    if report.committed {
        println!("committed local changes");
    } else {
        println!("no local changes to commit");
    }
    match report.integration {
        Integration::UpToDate => println!("up to date with origin"),
        Integration::FastForward => println!("fast-forwarded to origin"),
        Integration::Merged { conflicts: 0 } => println!("merged with origin"),
        Integration::Merged { conflicts } => {
            println!("merged with origin, {conflicts} conflict(s) resolved by last writer wins")
        }
    }
    println!("pushed to origin");
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
        let cli = Cli::try_parse_from(["boom", "cli", "config", "set", "user-name", "John"]);
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
            user_name: Some("John".into()),
            ..Default::default()
        };
        config::save(&path, &config).unwrap();

        let text = config_as_toml(&path).unwrap();
        assert!(!text.trim_start().starts_with('{'), "not TOML: {text}");
        let parsed: config::Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_gitrepo_create_with_remote() {
        let cli = Cli::try_parse_from([
            "boom",
            "cli",
            "gitrepo",
            "create",
            "/tmp/bookmarks",
            "--remote",
            "git@example.com:me/bookmarks.git",
        ]);
        assert!(cli.is_ok(), "{:?}", cli.err());
    }

    #[test]
    fn parses_gitrepo_set_remote() {
        let cli = Cli::try_parse_from([
            "boom",
            "cli",
            "gitrepo",
            "set-remote",
            "git@example.com:me/bookmarks.git",
        ]);
        assert!(cli.is_ok(), "{:?}", cli.err());
    }
}
