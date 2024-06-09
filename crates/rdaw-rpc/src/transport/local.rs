use async_channel::{Receiver, Sender};

use super::{ClientTransport, ServerTransport};
use crate::{ClientMessage, Protocol, ProtocolError, ServerMessage};

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

#[derive(Debug, Clone)]
pub struct LocalClientTransport<P: Protocol> {
    sender: Sender<ClientMessage<P>>,
    receiver: Receiver<ServerMessage<P>>,
}

impl<P: Protocol> ClientTransport<P> for LocalClientTransport<P> {
    async fn send(&self, message: ClientMessage<P>) -> Result<(), P::Error> {
        self.sender
            .send(message)
            .await
            .map_err(|_| P::Error::disconnected())
    }

    async fn recv(&self) -> Result<ServerMessage<P>, P::Error> {
        self.receiver
            .recv()
            .await
            .map_err(|_| P::Error::disconnected())
    }
}

#[derive(Debug, Clone)]
pub struct LocalServerTransport<P: Protocol> {
    sender: Sender<ServerMessage<P>>,
    receiver: Receiver<ClientMessage<P>>,
}

impl<P: Protocol> ServerTransport<P> for LocalServerTransport<P> {
    async fn send(&self, message: ServerMessage<P>) -> Result<(), P::Error> {
        self.sender
            .send(message)
            .await
            .map_err(|_| P::Error::disconnected())
    }

    async fn recv(&self) -> Result<ClientMessage<P>, P::Error> {
        self.receiver
            .recv()
            .await
            .map_err(|_| P::Error::disconnected())
    }
}
