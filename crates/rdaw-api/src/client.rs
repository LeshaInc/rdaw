use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use async_channel::{Receiver, Sender};
use crossbeam_queue::SegQueue;
use futures_lite::Stream;
use pin_project_lite::pin_project;
use rdaw_core::collections::{dashmap, DashMap};

use crate::transport::ClientTransport;
use crate::{ClientMessage, Error, EventStreamId, Protocol, RequestId, Result, ServerMessage};

#[derive(Debug)]
pub struct Client<P: Protocol, T: ClientTransport<P>> {
    inner: Arc<Inner<P, T>>,
}

#[derive(Debug)]
struct Inner<P: Protocol, T: ClientTransport<P>> {
    transport: T,
    req_counter: AtomicU64,
    requests: DashMap<RequestId, RequestSlot<P>>,
    streams: DashMap<EventStreamId, Sender<P::Event>>,
    closed_streams: SegQueue<EventStreamId>,
}

impl<P: Protocol, T: ClientTransport<P>> Client<P, T> {
    pub fn new(transport: T) -> Client<P, T> {
        Client {
            inner: Arc::new(Inner {
                transport,
                req_counter: AtomicU64::new(0),
                requests: DashMap::default(),
                streams: DashMap::default(),
                closed_streams: SegQueue::new(),
            }),
        }
    }

    pub async fn handle(&self) -> Result<()> {
        loop {
            let msg = match self.inner.transport.recv().await {
                Ok(v) => v,
                Err(Error::Disconnected) => return Ok(()),
                Err(e) => return Err(e),
            };

            self.handle_msg(msg).await;

            while let Some(id) = self.inner.closed_streams.pop() {
                self.inner
                    .transport
                    .send(ClientMessage::CloseStream { id })
                    .await?;
            }
        }
    }

    pub async fn request(&self, req: P::Req) -> Result<P::Res> {
        let id = RequestId(self.inner.req_counter.fetch_add(1, Ordering::Relaxed));

        let msg = ClientMessage::Request {
            id,
            payload: req.into(),
        };

        self.inner.transport.send(msg).await?;
        self.wait_for_response(id).await
    }

    pub fn subscribe(&self, id: EventStreamId) -> impl Stream<Item = P::Event> {
        let (sender, receiver) = async_channel::unbounded();
        self.inner.streams.insert(id, sender);

        EventStream {
            cleaner: EventStreamCleaner {
                id,
                inner: self.inner.clone(),
            },
            receiver,
        }
    }

    fn wait_for_response(&self, id: RequestId) -> impl Future<Output = Result<P::Res>> + '_ {
        std::future::poll_fn(move |ctx| {
            let mut slot = self
                .inner
                .requests
                .entry(id)
                .or_insert_with(|| RequestSlot {
                    response: None,
                    waker: None,
                });

            if let Some(response) = slot.response.take() {
                return Poll::Ready(response);
            }

            slot.waker = Some(ctx.waker().clone());

            Poll::Pending
        })
    }

    async fn handle_msg(&self, msg: ServerMessage<P>) {
        match msg {
            ServerMessage::Response { id, payload } => {
                let mut slot = self
                    .inner
                    .requests
                    .entry(id)
                    .or_insert_with(|| RequestSlot {
                        response: None,
                        waker: None,
                    });

                slot.response = Some(payload);

                if let Some(waker) = slot.waker.take() {
                    waker.wake();
                }
            }

            ServerMessage::Event { id, payload } => {
                let dashmap::mapref::entry::Entry::Occupied(mut entry) =
                    self.inner.streams.entry(id)
                else {
                    return;
                };

                let res = entry.get_mut().send_blocking(payload);
                if res.is_err() {
                    entry.remove();
                }
            }

            ServerMessage::CloseStream { id } => {
                self.inner.streams.remove(&id);
            }
        }
    }
}

#[derive(Debug)]
struct RequestSlot<P: Protocol> {
    response: Option<Result<P::Res>>,
    waker: Option<Waker>,
}

pin_project! {
    struct EventStream<P: Protocol, T: ClientTransport<P>> {
        cleaner: EventStreamCleaner<P, T>,
        #[pin]
        receiver: Receiver<P::Event>,
    }
}

impl<P: Protocol, T: ClientTransport<P>> Stream for EventStream<P, T> {
    type Item = P::Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<P::Event>> {
        self.project().receiver.poll_next(cx)
    }
}

struct EventStreamCleaner<P: Protocol, T: ClientTransport<P>> {
    id: EventStreamId,
    inner: Arc<Inner<P, T>>,
}

impl<P: Protocol, T: ClientTransport<P>> Drop for EventStreamCleaner<P, T> {
    fn drop(&mut self) {
        self.inner.closed_streams.push(self.id);
    }
}
