//! Command-line interface, invoked as `boom cli <command>`.

use clap::{Parser, Subcommand};

use crate::config::{self, Config};
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
    /// Inspect the config.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the current config as TOML.
    Show,
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
    let config = match config::load(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: failed to read {}: {err}", config_path.display());
            return 1;
        }
    };

    match cli.command {
        TopCommand::Cli { command } => match command {
            CliCommand::Ping => dispatch(Action::Ping, &config),
            CliCommand::Config {
                command: ConfigCommand::Show,
            } => {
                print_config(&config);
                0
            }
        },
    }
}

fn dispatch(action: Action, config: &Config) -> i32 {
    let response = protocol::handle(
        Request {
            id: "cli".into(),
            action,
        },
        config,
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

fn print_config(config: &Config) {
    match toml::to_string_pretty(config) {
        Ok(text) => print!("{text}"),
        Err(err) => eprintln!("error: {err}"),
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
}
