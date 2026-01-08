//! 错误类型定义。

use std::fmt;

#[derive(Debug)]
pub enum Error {
    QueueNotFound(String),
    QueueAlreadyExists(String),
    RapidError(nnxt_rapid::Error),
    ControlNotAvailable,
    NngError(nng::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::QueueNotFound(addr) => write!(f, "queue not found: {}", addr),
            Error::QueueAlreadyExists(addr) => write!(f, "queue already exists: {}", addr),
            Error::RapidError(e) => write!(f, "rapid error: {}", e),
            Error::ControlNotAvailable => write!(f, "control socket not available"),
            Error::NngError(e) => write!(f, "nng error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<nnxt_rapid::Error> for Error {
    fn from(e: nnxt_rapid::Error) -> Self {
        Error::RapidError(e)
    }
}

impl From<nng::Error> for Error {
    fn from(e: nng::Error) -> Self {
        Error::NngError(e)
    }
}
