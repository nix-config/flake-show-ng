//! The single error type and its stderr report.
//!
//! No anyhow: every failure is one of four variants, and every user-facing
//! stderr line starts with the same prefix (see [`stderr_prefix`]).  Every
//! variant is a runtime failure, so `main` maps it to exit code 1 directly;
//! usage exit 2 comes from clap itself during parsing.

use std::fmt;

/// The one prefix used by the error report, the swallow notice and the
/// unknown-part notice, so all three stay byte-identical.
pub fn stderr_prefix() -> &'static str {
    "fsn: "
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Message {
        message: String,
    },
    /// An error that has already been reported to stderr in detail.
    Silent,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::Message { message } => f.write_str(message),
            Error::Silent => Ok(()),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Error::Message { message: m.into() }
    }

    /// Print the `fsn: `-prefixed report, except for [`Error::Silent`] which
    /// was already reported by the caller.
    pub fn report(&self) {
        match self {
            Error::Silent => {}
            other => eprintln!("{}{other}", stderr_prefix()),
        }
    }
}
