use async_channel::{Receiver, Sender};

use crate::{ClientMessage, Error, Protocol, Result, ServerMessage};

#[trait_variant::make(Send)]
pub trait ClientTransport<P: Protocol> {
    async fn send(&self, message: ClientMessage<P>) -> Result<()>;

    async fn recv(&self) -> Result<ServerMessage<P>>;
}

#[trait_variant::make(Send)]
pub trait ServerTransport<P: Protocol> {
    async fn send(&self, message: ServerMessage<P>) -> Result<()>;

    async fn recv(&self) -> Result<ClientMessage<P>>;
}

#[derive(Debug)]
pub struct LocalClientTransport<P: Protocol> {
    sender: Sender<ClientMessage<P>>,
    receiver: Receiver<ServerMessage<P>>,
}

impl<P: Protocol> ClientTransport<P> for LocalClientTransport<P> {
    async fn send(&self, message: ClientMessage<P>) -> Result<()> {
        self.sender
            .send(message)
            .await
            .map_err(|_| Error::Disconnected)
    }

    async fn recv(&self) -> Result<ServerMessage<P>> {
        self.receiver.recv().await.map_err(|_| Error::Disconnected)
    }
}

#[derive(Debug)]
pub struct LocalServerTransport<P: Protocol> {
    sender: Sender<ServerMessage<P>>,
    receiver: Receiver<ClientMessage<P>>,
}

impl<P: Protocol> ServerTransport<P> for LocalServerTransport<P> {
    async fn send(&self, message: ServerMessage<P>) -> Result<()> {
        self.sender
            .send(message)
            .await
            .map_err(|_| Error::Disconnected)
    }

    async fn recv(&self) -> Result<ClientMessage<P>> {
        self.receiver.recv().await.map_err(|_| Error::Disconnected)
    }
}

pub fn local<P: Protocol>(
    cap: Option<usize>,
) -> (LocalClientTransport<P>, LocalServerTransport<P>) {
    let ((client_sender, server_receiver), (server_sender, client_receiver)) =
        if let Some(cap) = cap {
            (async_channel::bounded(cap), async_channel::bounded(cap))
        } else {
            (async_channel::unbounded(), async_channel::unbounded())
        };

    (
        LocalClientTransport {
            sender: client_sender,
            receiver: client_receiver,
        },
        LocalServerTransport {
            sender: server_sender,
            receiver: server_receiver,
        },
    )
}
