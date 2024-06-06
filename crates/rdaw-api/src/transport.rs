use async_channel::{Receiver, Sender};

use crate::{ClientMessage, Error, Result, ServerMessage};

#[trait_variant::make(Send)]
pub trait ClientTransport<Req, Res, Event> {
    async fn send(&self, message: ClientMessage<Req>) -> Result<()>;

    async fn recv(&self) -> Result<ServerMessage<Res, Event>>;
}

#[trait_variant::make(Send)]
pub trait ServerTransport<Req, Res, Event> {
    async fn send(&self, message: ServerMessage<Res, Event>) -> Result<()>;

    async fn recv(&self) -> Result<ClientMessage<Req>>;
}

#[derive(Debug)]
pub struct LocalClientTransport<Req, Res, Event> {
    sender: Sender<ClientMessage<Req>>,
    receiver: Receiver<ServerMessage<Res, Event>>,
}

impl<Req: Send, Res: Send, Event: Send> ClientTransport<Req, Res, Event>
    for LocalClientTransport<Req, Res, Event>
{
    async fn send(&self, message: ClientMessage<Req>) -> Result<()> {
        self.sender
            .send(message)
            .await
            .map_err(|_| Error::Disconnected)
    }

    async fn recv(&self) -> Result<ServerMessage<Res, Event>> {
        self.receiver.recv().await.map_err(|_| Error::Disconnected)
    }
}

#[derive(Debug)]
pub struct LocalServerTransport<Req, Res, Event> {
    sender: Sender<ServerMessage<Res, Event>>,
    receiver: Receiver<ClientMessage<Req>>,
}

impl<Req: Send, Res: Send, Event: Send> ServerTransport<Req, Res, Event>
    for LocalServerTransport<Req, Res, Event>
{
    async fn send(&self, message: ServerMessage<Res, Event>) -> Result<()> {
        self.sender
            .send(message)
            .await
            .map_err(|_| Error::Disconnected)
    }

    async fn recv(&self) -> Result<ClientMessage<Req>> {
        self.receiver.recv().await.map_err(|_| Error::Disconnected)
    }
}

pub fn local<Req: Send, Res: Send, Event: Send>(
    cap: Option<usize>,
) -> (
    LocalClientTransport<Req, Res, Event>,
    LocalServerTransport<Req, Res, Event>,
) {
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
