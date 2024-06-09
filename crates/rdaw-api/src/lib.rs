#![allow(clippy::type_complexity)]

pub mod arrangement;
pub mod audio;
pub mod blob;
mod error;
pub mod item;
pub mod source;
pub mod tempo_map;
#[cfg(test)]
mod tests;
pub mod time;
pub mod track;

use std::fmt::Debug;
use std::pin::Pin;

use futures::Stream;

pub use self::error::{Error, Result};

pub trait Backend:
    self::arrangement::ArrangementOperations
    + self::source::AudioSourceOperations
    + self::blob::BlobOperations
    + self::track::TrackOperations
    + Sync
    + 'static
{
}

impl<T> Backend for T where
    T: self::arrangement::ArrangementOperations
        + self::source::AudioSourceOperations
        + self::blob::BlobOperations
        + self::track::TrackOperations
        + Sync
        + 'static
{
}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

#[rdaw_rpc::protocol(
    operations(
        self::arrangement::ArrangementOperations,
        self::source::AudioSourceOperations,
        self::blob::BlobOperations,
        self::track::TrackOperations
    ),
    error = Error
)]
#[derive(Debug, Clone, Copy)]
pub struct BackendProtocol;
