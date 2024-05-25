use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("disconnected")]
    Disconnected,
    #[error("invalid ID")]
    InvalidId,
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
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
