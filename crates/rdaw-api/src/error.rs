use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("disconnected")]
    Disconnected,
    #[error("invalid ID")]
    InvalidId,
    #[error("invalid type")]
    InvalidType,
    #[error("index out of bounds")]
    IndexOutOfBounds,
    #[error("recursive tracks are not supported")]
    RecursiveTrack,
    #[error("filesystem error: {path}: {error}")]
    Filesystem {
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
    #[error("internal error: {error}")]
    Internal {
        #[source]
        error: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl Error {
    #[cold]
    pub fn new_internal<E: std::error::Error + Send + Sync + 'static>(error: E) -> Error {
        Error::Internal {
            error: Box::new(error),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
