use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Disconnected")]
    Disconnected,
    #[error("invalid ID")]
    InvalidId,
    #[error("index out of bounds")]
    IndexOutOfBounds,
    #[error("filesystem error: {path}: {error}")]
    Filesystem {
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
