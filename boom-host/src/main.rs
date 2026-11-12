//! Native messaging host and CLI for boom.
//!
//! The browser starts `boom` with the extension origin as an argument and
//! talks to it over stdin/stdout. Any other invocation of the form
//! `boom cli <command>` runs the CLI.

mod cli;
mod config;
mod gitrepo;
mod native;
mod ops;
mod protocol;

use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::protocol::Request;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("cli") {
        let cli = Cli::parse();
        return ExitCode::from(cli::run(cli) as u8);
    }

    match serve_native() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("boom: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Serves native messaging requests until the browser closes stdin.
/// Each request is handled on its own, so the host stays stateless.
fn serve_native() -> io::Result<()> {
    let config_path = config::default_path()?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    while let Some(body) = native::read_message(&mut input)? {
        let response = match serde_json::from_slice::<Request>(&body) {
            Ok(request) => protocol::handle(request, &config_path),
            Err(err) => protocol::Response {
                id: String::new(),
                ok: false,
                result: None,
                error: Some(format!("invalid request: {err}")),
            },
        };
        native::write_message(&mut output, &response)?;
        output.flush()?;
    }
    Ok(())
}
