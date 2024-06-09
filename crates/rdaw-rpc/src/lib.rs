mod client;
mod id_allocator;
mod subscribers;
pub mod transport;

pub use rdaw_macros::{
    rpc_handler as handler, rpc_operations as operations, rpc_protocol as protocol,
};

pub use self::client::Client;
pub use self::id_allocator::IdAllocator;
pub use self::subscribers::Subscribers;

pub trait Protocol: Send + Sync + 'static {
    type Req: Send + 'static;
    type Res: Send + 'static;
    type Event: Send + 'static;
    type Error: ProtocolError;
}

pub trait ProtocolError: std::error::Error + Send + 'static {
    fn disconnected() -> Self;

    fn invalid_type() -> Self;

    fn is_disconnected(&self) -> bool;

    fn is_invalid_type(&self) -> bool;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RequestId(pub u64);

impl From<u64> for RequestId {
    fn from(value: u64) -> Self {
        RequestId(value)
    }
}

pub type RequestIdAllocator = IdAllocator<RequestId>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct StreamId(pub u64);

impl From<u64> for StreamId {
    fn from(value: u64) -> Self {
        StreamId(value)
    }
}

pub type StreamIdAllocator = IdAllocator<StreamId>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ClientMessage<P: Protocol> {
    Request { id: RequestId, payload: P::Req },
    CloseStream { id: StreamId },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ServerMessage<P: Protocol> {
    Response {
        id: RequestId,
        payload: Result<P::Res, P::Error>,
    },
    Event {
        id: StreamId,
        payload: P::Event,
    },
    CloseStream {
        id: StreamId,
    },
}
