pub mod arrangement;
pub mod blob;
mod error;
pub mod item;
pub mod source;
pub mod tempo_map;
pub mod time;
pub mod track;

use std::pin::Pin;

use futures_lite::Stream;

use self::blob::BlobOperations;
pub use self::error::{Error, Result};
use self::track::TrackOperations;

pub trait Backend: TrackOperations + BlobOperations + Sync + 'static {}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;
