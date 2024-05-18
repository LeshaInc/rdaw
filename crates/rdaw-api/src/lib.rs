mod beat;
mod blob;
mod error;
mod item;
mod source;
mod track;

use std::pin::Pin;

use futures_lite::Stream;

pub use self::beat::{BeatTime, Time};
pub use self::blob::{BlobId, BlobOperations};
pub use self::error::{Error, Result};
pub use self::item::{AudioItemId, ItemId};
pub use self::source::{AudioSourceId, SourceId};
pub use self::track::{TrackEvent, TrackId, TrackItem, TrackItemId, TrackOperations};

pub trait Backend: TrackOperations + BlobOperations + Sync + 'static {}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;
