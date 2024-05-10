use std::io;

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to spawn thread")]
    ThreadSpawn(#[source] io::Error),

    #[error("thread crashed")]
    ThreadCrashed,

    #[error("invalid stream ID")]
    InvalidStreamId,

    #[error("too many channels")]
    TooManyChannels,

    #[error(transparent)]
    Pipewire(#[from] pipewire::Error),

    #[error(transparent)]
    Spa(#[from] pipewire::spa::utils::result::Error),

    #[error("serialization error")]
    Serialization(#[from] pipewire::spa::pod::serialize::GenError),
}

pub type Result<T> = std::result::Result<T, Error>;
