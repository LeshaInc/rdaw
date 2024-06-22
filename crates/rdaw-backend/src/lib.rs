pub mod arrangement;
pub mod asset;
pub mod document;
pub mod item;
pub mod object;
pub mod source;
pub mod tempo_map;
#[cfg(test)]
pub mod tests;
pub mod track;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use document::DocumentStorage;
use futures::executor::ThreadPool;
use futures::{select_biased, FutureExt};
use rdaw_api::{BackendProtocol, BackendRequest, ErrorKind, Result};
use rdaw_rpc::transport::{LocalServerTransport, ServerTransport};
use rdaw_rpc::{ClientMessage, StreamIdAllocator};

use self::object::{Hub, SubscribersHub};
use self::track::TrackViewCache;

#[derive(Debug)]
pub struct Backend {
    transport: LocalServerTransport<BackendProtocol>,

    thread_pool: ThreadPool,
    queue: DeferredQueue,

    documents: DocumentStorage,
    hub: Hub,
    subscribers: SubscribersHub,

    track_view_cache: TrackViewCache,
}

impl Backend {
    pub fn new(transport: LocalServerTransport<BackendProtocol>) -> Backend {
        let stream_id_allocator = Arc::new(StreamIdAllocator::new());

        Backend {
            transport,

            thread_pool: ThreadPool::new().unwrap(),
            queue: DeferredQueue::new(),

            documents: DocumentStorage::default(),
            hub: Hub::default(),
            subscribers: SubscribersHub::new(stream_id_allocator.clone()),

            track_view_cache: TrackViewCache::default(),
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        self.subscribers.deliver(&self.transport).await?;
        Ok(())
    }

    pub async fn handle(&mut self) -> Result<()> {
        loop {
            let msg = select_biased! {
                task = self.queue.receiver.recv().fuse() => {
                    if let Ok(task) = task {
                        task(self).await?;
                    }
                    continue
                }

                msg = self.transport.recv().fuse() => match msg {
                    Ok(v) => v,
                    Err(e) if e.kind() == ErrorKind::Disconnected => return Ok(()),
                    Err(e) => return Err(e),
                }
            };

            match msg {
                ClientMessage::Request { id, payload } => match payload {
                    BackendRequest::Arrangement(req) => {
                        self.handle_arrangement_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::Asset(req) => {
                        self.handle_asset_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::AudioSource(req) => {
                        self.handle_audio_source_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::Document(req) => {
                        self.handle_document_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::Track(req) => {
                        self.handle_track_request(self.transport.clone(), id, req)
                            .await?
                    }
                },
                ClientMessage::CloseStream { id } => self.subscribers.close_one(id),
            }

            self.update().await?;
        }
    }

    fn spawn(&self, fut: impl Future<Output = Result<()>> + Send + 'static) {
        self.thread_pool.spawn_ok(async move {
            if let Err(error) = fut.await {
                tracing::error!(?error);
            }
        })
    }
}

pub trait DeferredTask: Send + 'static {
    fn run(self, backend: &mut Backend) -> impl Future<Output = Result<()>> + Send;
}

impl<Fn, Fut> DeferredTask for Fn
where
    Fn: FnOnce(&mut Backend) -> Fut,
    Fn: Send + 'static,
    Fut: Send + Future<Output = Result<()>>,
{
    fn run(self, backend: &mut Backend) -> impl Future<Output = Result<()>> {
        self(backend)
    }
}

type BoxedDeferredTask = Box<
    dyn (for<'a> FnOnce(&'a mut Backend) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>)
        + Send,
>;

#[derive(Debug, Clone)]
pub struct DeferredQueue {
    sender: Sender<BoxedDeferredTask>,
    receiver: Receiver<BoxedDeferredTask>,
}

impl DeferredQueue {
    fn new() -> Self {
        let (sender, receiver) = async_channel::unbounded();
        Self { sender, receiver }
    }

    pub fn defer(&self, task: impl DeferredTask) {
        self.sender
            .send_blocking(Box::new(move |b| Box::pin(task.run(b))))
            .unwrap();
    }
}
