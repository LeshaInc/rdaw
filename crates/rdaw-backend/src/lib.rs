pub mod arrangement;
pub mod blob;
pub mod document;
pub mod item;
pub mod object;
pub mod source;
pub mod tempo_map;
#[cfg(test)]
pub mod tests;
pub mod track;

use std::sync::Arc;

use rdaw_api::{BackendProtocol, BackendRequest, Error, Result};
use rdaw_rpc::transport::{LocalServerTransport, ServerTransport};
use rdaw_rpc::{ClientMessage, StreamIdAllocator};

use self::blob::BlobCache;
use self::object::{Hub, SubscribersHub};
use self::track::TrackViewCache;

#[derive(Debug)]
pub struct Backend {
    transport: LocalServerTransport<BackendProtocol>,

    hub: Hub,
    subscribers: SubscribersHub,

    blob_cache: BlobCache,
    track_view_cache: TrackViewCache,
}

impl Backend {
    pub fn new(transport: LocalServerTransport<BackendProtocol>) -> Backend {
        let stream_id_allocator = Arc::new(StreamIdAllocator::new());

        Backend {
            transport,

            hub: Hub::default(),
            subscribers: SubscribersHub::new(stream_id_allocator.clone()),

            blob_cache: BlobCache::default(),
            track_view_cache: TrackViewCache::default(),
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        self.subscribers.deliver(&self.transport).await?;
        Ok(())
    }

    pub async fn handle(&mut self) -> Result<()> {
        loop {
            let msg = match self.transport.recv().await {
                Ok(v) => v,
                Err(Error::Disconnected) => return Ok(()),
                Err(e) => return Err(e),
            };

            match msg {
                ClientMessage::Request { id, payload } => match payload {
                    BackendRequest::Arrangement(req) => {
                        self.handle_arrangement_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::AudioSource(req) => {
                        self.handle_audio_source_request(self.transport.clone(), id, req)
                            .await?
                    }
                    BackendRequest::Blob(req) => {
                        self.handle_blob_request(self.transport.clone(), id, req)
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
}
