use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::task::{Poll, Waker};

use futures_lite::FutureExt;
use rdaw_core::collections::HashMap;

use crate::transport::ClientTransport;
use crate::{ClientMessage, Protocol, RequestId, Result, ServerMessage};

#[derive(Debug)]
pub struct Client<P: Protocol, T: ClientTransport<P>> {
    transport: T,
    req_counter: AtomicU64,
    requests: Mutex<HashMap<RequestId, RequestSlot<P>>>,
}

impl<P: Protocol, T: ClientTransport<P>> Client<P, T> {
    pub fn new(transport: T) -> Client<P, T> {
        Client {
            transport,
            req_counter: AtomicU64::new(0),
            requests: Mutex::default(),
        }
    }

    pub async fn request(&self, req: P::Req) -> Result<P::Res> {
        let id = RequestId(self.req_counter.fetch_add(1, Ordering::Relaxed));

        let msg = ClientMessage::Request {
            id,
            payload: req.into(),
        };

        self.transport.send(msg).await?;

        let recv = async {
            loop {
                self.recv().await?;
            }
        };

        self.wait_for_response(id).or(recv).await
    }

    fn wait_for_response(&self, id: RequestId) -> impl Future<Output = Result<P::Res>> + '_ {
        std::future::poll_fn(move |ctx| {
            let Ok(mut requests) = self.requests.lock() else {
                return Poll::Pending;
            };

            let slot = requests.entry(id).or_insert_with(|| RequestSlot {
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

    async fn recv(&self) -> Result<()> {
        let msg = self.transport.recv().await?;
        self.dispatch(msg);
        Ok(())
    }

    fn dispatch(&self, msg: ServerMessage<P>) {
        match msg {
            ServerMessage::Response { id, payload } => {
                let Ok(mut requests) = self.requests.lock() else {
                    return;
                };

                let slot = requests.entry(id).or_insert_with(|| RequestSlot {
                    response: None,
                    waker: None,
                });

                slot.response = Some(payload);

                if let Some(waker) = slot.waker.take() {
                    waker.wake();
                }
            }

            ServerMessage::Event { .. } => todo!(),
        }
    }
}

#[derive(Debug)]
struct RequestSlot<P: Protocol> {
    response: Option<Result<P::Res>>,
    waker: Option<Waker>,
}
