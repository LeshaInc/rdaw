use std::fmt::Display;
use std::path::Path;

use rdaw_rpc::ProtocolError;

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("internal error: {message}")]
    Internal { message: String },

    #[error("io error: {message}")]
    Io { message: String },
    #[error("filesystem error: {path}: {message}")]
    Filesystem { path: String, message: String },

    #[error("disconnected")]
    Disconnected,
    #[error("index out of bounds")]
    IndexOutOfBounds,
    #[error("invalid ID")]
    InvalidId,
    #[error("invalid type")]
    InvalidType,
    #[error("recursive tracks are not supported")]
    RecursiveTrack,
}

impl Error {
    #[cold]
    pub fn new_internal<E: Display>(error: E) -> Error {
        Error::Internal {
            message: error.to_string(),
        }
    }

    #[cold]
    pub fn new_io<E: Display>(error: E) -> Error {
        Error::Io {
            message: error.to_string(),
        }
    }

    #[cold]
    pub fn new_filesystem<P: AsRef<Path>, E: Display>(path: P, error: E) -> Error {
        Error::Filesystem {
            path: path.as_ref().to_string_lossy().into_owned(),
            message: error.to_string(),
        }
    }
}

impl ProtocolError for Error {
    fn disconnected() -> Self {
        Error::Disconnected
    }

    fn invalid_type() -> Self {
        Error::InvalidType
    }

    fn is_disconnected(&self) -> bool {
        matches!(self, Error::Disconnected)
    }

    fn is_invalid_type(&self) -> bool {
        matches!(self, Error::InvalidType)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
