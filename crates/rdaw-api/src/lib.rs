pub mod arrangement;
pub mod audio;
pub mod blob;
mod error;
pub mod item;
pub mod source;
pub mod tempo_map;
pub mod time;
pub mod track;
pub mod transport;

use std::pin::Pin;

use futures_lite::Stream;

use self::arrangement::ArrangementOperations;
use self::blob::BlobOperations;
pub use self::error::{Error, Result};
use self::source::AudioSourceOperations;
use self::track::TrackOperations;

pub trait Backend:
    ArrangementOperations + AudioSourceOperations + BlobOperations + TrackOperations + Sync + 'static
{
}

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct EventStreamId(pub u64);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RequestId(pub u64);

#[derive(Debug)]
pub enum ClientMessage<Req> {
    Request { id: RequestId, payload: Req },
}

#[derive(Debug)]
pub enum ServerMessage<Res, Event> {
    Response { id: RequestId, payload: Res },
    Event { id: EventStreamId, payload: Event },
}
