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
    type Req: Send + Debug;
    type Res: Send + Debug;
    type Event: Send + Debug;
}

#[derive(Debug, Clone, Copy)]
pub struct BackendProtocol;

impl Protocol for BackendProtocol {
    type Req = BackendRequest;
    type Res = BackendResponse;
    type Event = BackendEvents;
}

#[derive(Debug, Clone)]
pub enum BackendRequest {
    Arrangement(self::arrangement::ArrangementRequest),
    AudioSource(self::source::AudioSourceRequest),
    Blob(self::blob::BlobRequest),
    Track(self::track::TrackRequest),
}

#[derive(Debug, Clone)]
pub enum BackendResponse {
    Arrangement(self::arrangement::ArrangementResponse),
    AudioSource(self::source::AudioSourceResponse),
    Blob(self::blob::BlobResponse),
    Track(self::track::TrackResponse),
}

#[derive(Debug, Clone)]
pub enum BackendEvents {
    Arrangement(self::arrangement::ArrangementEvents),
    AudioSource(self::source::AudioSourceEvents),
    Blob(self::blob::BlobEvents),
    Track(self::track::TrackEvents),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct EventStreamId(pub u64);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RequestId(pub u64);

#[derive(Debug)]
pub enum ClientMessage<P: Protocol> {
    Request { id: RequestId, payload: P::Req },
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
}
