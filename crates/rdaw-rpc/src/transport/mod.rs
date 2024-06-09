mod local;

pub use self::local::{local, LocalClientTransport, LocalServerTransport};
use crate::{ClientMessage, Protocol, ServerMessage};

#[trait_variant::make(Send)]
pub trait ClientTransport<P: Protocol>: Clone + Send + Sync + 'static {
    async fn send(&self, message: ClientMessage<P>) -> Result<(), P::Error>;

    async fn recv(&self) -> Result<ServerMessage<P>, P::Error>;
}

#[trait_variant::make(Send)]
pub trait ServerTransport<P: Protocol>: Clone + Send + Sync + 'static {
    async fn send(&self, message: ServerMessage<P>) -> Result<(), P::Error>;

    async fn recv(&self) -> Result<ClientMessage<P>, P::Error>;
}
