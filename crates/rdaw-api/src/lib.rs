#![allow(clippy::type_complexity)]

pub mod arrangement;
pub mod asset;
pub mod audio;
pub mod document;
pub mod error;
pub mod item;
pub mod media;
pub mod source;
pub mod tempo_map;
#[cfg(test)]
mod tests;
pub mod time;
pub mod track;

use std::fmt::Debug;
use std::pin::Pin;

use futures::Stream;

pub use self::error::{Error, ErrorKind, Result};

pub trait Backend:
    self::arrangement::ArrangementOperations
    + self::asset::AssetOperations
    + self::source::AudioSourceOperations
    + self::document::DocumentOperations
    + self::track::TrackOperations
    + Sync
    + 'static
{
}

impl<T> Backend for T where
    T: self::arrangement::ArrangementOperations
        + self::asset::AssetOperations
        + self::source::AudioSourceOperations
        + self::document::DocumentOperations
        + self::track::TrackOperations
        + Sync
        + 'static
{
}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

#[rdaw_rpc::protocol(
    operations(
        self::arrangement::ArrangementOperations,
        self::asset::AssetOperations,
        self::source::AudioSourceOperations,
        self::document::DocumentOperations,
        self::track::TrackOperations
    ),
    error = Error
)]
#[derive(Debug, Clone, Copy)]
pub struct BackendProtocol;
