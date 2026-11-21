use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;
use flake_show_ng::cli::Cli;
use flake_show_ng::error::Error;
use flake_show_ng::pipeline;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let flake = cli.flake.clone();
    match pipeline::run(cli.into_filters(), &flake).and_then(write_stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Clap reserves exit code 2 for usage errors.
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
