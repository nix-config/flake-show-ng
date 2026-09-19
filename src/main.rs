//! `fsn` — show the complete tree of a flake's outputs.

use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;
use flake_show_ng::cli::Cli;
use flake_show_ng::error::Error;
use flake_show_ng::pipeline;

fn main() -> ExitCode {
    match pipeline::run(Cli::parse().into_args()).and_then(write_stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Runtime failures exit 1; exit code 2 is produced by clap itself.
            error.report();
            ExitCode::from(1u8)
        }
    }
}

fn write_stdout(text: String) -> Result<(), Error> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        // `fsn | head` closes the pipe early; a broken pipe is not a failure.
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e.into()),
    }
}
