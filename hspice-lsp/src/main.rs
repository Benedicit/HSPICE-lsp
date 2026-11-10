//! HSPICE language server entry point.

#![forbid(unsafe_code)]

mod analysis;
mod config;
mod features;
mod lang;
mod server;
mod state;
mod syntax;
#[cfg(test)]
mod test_support;

use std::process::ExitCode;

/// Starts the server on stdio, or prints the version for `--version`.
fn main() -> ExitCode {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (connection, io_threads) = lsp_server::Connection::stdio();
    let outcome = server::run(&connection).and_then(|()| {
        drop(connection);
        io_threads.join().map_err(Into::into)
    });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hspice-lsp: {error}");
            ExitCode::FAILURE
        }
    }
}
