//! No anyhow: every failure is one of four variants and every user-facing line
//! shares one prefix; clap owns exit code 2, so all variants map to 1.

use std::fmt;

/// The one prefix shared by the error report and both notices, so they stay
/// byte-identical.
pub fn stderr_prefix() -> &'static str {
    "fsn: "
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Message { message: String },
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

    pub fn report(&self) {
        match self {
            Error::Silent => {}
            other => eprintln!("{}{other}", stderr_prefix()),
        }
    }
}
