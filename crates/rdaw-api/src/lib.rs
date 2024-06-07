pub mod arrangement;
pub mod audio;
pub mod blob;
mod client;
mod error;
pub mod item;
pub mod source;
pub mod tempo_map;
pub mod time;
pub mod track;
pub mod transport;

use std::fmt::Debug;
use std::pin::Pin;

use futures_lite::Stream;

pub use self::client::Client;
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

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

pub trait Protocol: Send + Sync + Copy + Debug + 'static {
    type Req: Send + Debug + 'static;
    type Res: Send + Debug + 'static;
    type Event: Send + Debug + 'static;
}

#[rdaw_macros::api_protocol(
    self::arrangement::ArrangementOperations,
    self::source::AudioSourceOperations,
    self::blob::BlobOperations,
    self::track::TrackOperations
)]
#[derive(Debug, Clone, Copy)]
pub struct BackendProtocol;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct EventStreamId(pub u64);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RequestId(pub u64);

#[derive(Debug)]
pub enum ClientMessage<P: Protocol> {
    Request { id: RequestId, payload: P::Req },
    CloseStream { id: EventStreamId },
}

#[derive(Debug)]
pub enum ServerMessage<P: Protocol> {
    Response {
        id: RequestId,
        payload: Result<P::Res>,
    },
    Event {
        id: EventStreamId,
        payload: P::Event,
    },
    CloseStream {
        id: EventStreamId,
    },
}
