mod blob;
mod dispatch;
mod subscribers;
mod track;

use async_channel::{Receiver, Sender};
use rdaw_api::{Operations, TrackEvent};
use rdaw_object::{Hub, TrackId};

pub use self::blob::{BlobCache, BlobOperation};
pub use self::subscribers::{Subscriber, Subscribers};
pub use self::track::TrackOperation;

#[derive(Debug)]
pub struct Backend {
    hub: Hub,
    blob_cache: BlobCache,
    track_subscribers: Subscribers<TrackId, TrackEvent>,
    sender: Sender<Operation>,
    receiver: Receiver<Operation>,
}

impl Backend {
    pub fn new() -> Backend {
        let (sender, receiver) = async_channel::unbounded();
        Backend {
            hub: Hub::default(),
            blob_cache: BlobCache::default(),
            track_subscribers: Subscribers::default(),
            sender,
            receiver,
        }
    }

    pub fn handle(&self) -> BackendHandle {
        BackendHandle {
            sender: self.sender.clone(),
        }
    }

    pub async fn dispatch(&mut self, operation: Operation) {
        match operation {
            Operation::Track(op) => self.dispatch_track_operation(op).await,
            Operation::Blob(op) => self.dispatch_blob_operation(op).await,
        }
    }

    pub fn cleanup(&mut self) {
        self.track_subscribers.cleanup();
    }

    pub async fn run(mut self) {
        loop {
            let Ok(op) = self.receiver.recv().await else {
                break;
            };

            self.dispatch(op).await;
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        Backend::new()
    }
}

#[derive(Debug)]
pub struct BackendHandle {
    sender: Sender<Operation>,
}

impl Operations for BackendHandle {}

#[derive(Debug)]
pub enum Operation {
    Track(TrackOperation),
    Blob(BlobOperation),
}

impl From<TrackOperation> for Operation {
    fn from(op: TrackOperation) -> Operation {
        Operation::Track(op)
    }
}

impl From<BlobOperation> for Operation {
    fn from(op: BlobOperation) -> Operation {
        Operation::Blob(op)
    }
}
