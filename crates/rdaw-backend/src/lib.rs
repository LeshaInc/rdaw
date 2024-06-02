pub mod arrangement;
pub mod blob;
pub mod dispatch;
pub mod item;
pub mod source;
pub mod storage;
pub mod subscribers;
pub mod tempo_map;
pub mod track;

use arrangement::ArrangementOperation;
use async_channel::{Receiver, Sender};
use rdaw_core::Uuid;
use slotmap::Key;

use self::blob::{BlobCache, BlobOperation};
use self::storage::Hub;
use self::subscribers::SubscribersHub;
use self::track::TrackOperation;

pub trait Object {
    type Id: Key;

    fn uuid(&self) -> Uuid;

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        let _unused = hub;
        callback(self.uuid());
    }
}

#[derive(Debug)]
pub struct Backend {
    sender: Sender<Operation>,
    receiver: Receiver<Operation>,
    hub: Hub,
    subscribers: SubscribersHub,
    blob_cache: BlobCache,
}

impl Backend {
    pub fn new() -> Backend {
        let (sender, receiver) = async_channel::unbounded();
        Backend {
            sender,
            receiver,
            hub: Hub::default(),
            subscribers: SubscribersHub::default(),
            blob_cache: BlobCache::default(),
        }
    }

    pub fn handle(&self) -> BackendHandle {
        BackendHandle {
            sender: self.sender.clone(),
        }
    }

    pub fn dispatch(&mut self, operation: Operation) {
        match operation {
            Operation::Arrangement(op) => self.dispatch_arrangement_operation(op),
            Operation::Blob(op) => self.dispatch_blob_operation(op),
            Operation::Track(op) => self.dispatch_track_operation(op),
        }
    }

    pub async fn update(&mut self) {
        self.subscribers.update().await;
    }

    pub async fn run(mut self) {
        loop {
            let Ok(op) = self.receiver.recv().await else {
                break;
            };

            self.dispatch(op);
            self.update().await;
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

impl rdaw_api::Backend for BackendHandle {}

pub enum Operation {
    Arrangement(ArrangementOperation),
    Blob(BlobOperation),
    Track(TrackOperation),
}

impl From<ArrangementOperation> for Operation {
    fn from(op: ArrangementOperation) -> Operation {
        Operation::Arrangement(op)
    }
}

impl From<BlobOperation> for Operation {
    fn from(op: BlobOperation) -> Operation {
        Operation::Blob(op)
    }
}

impl From<TrackOperation> for Operation {
    fn from(op: TrackOperation) -> Operation {
        Operation::Track(op)
    }
}
