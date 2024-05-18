use std::path::PathBuf;
use std::pin::Pin;

use futures_lite::Stream;
use rdaw_core::time::RealTime;
use rdaw_object::{BlobId, ItemId, Time, TrackId, TrackItem, TrackItemId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Disconnected")]
    Disconnected,
    #[error("invalid ID")]
    InvalidId,
    #[error("filesystem error: {path}: {error}")]
    Filesystem {
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

pub trait Backend: TrackOperations + BlobOperations + Sync + 'static {}

#[trait_variant::make(Send)]
pub trait TrackOperations {
    async fn list_tracks(&self) -> Result<Vec<TrackId>>;

    async fn create_track(&self, name: String) -> Result<TrackId>;

    async fn subscribe_track(&self, id: TrackId) -> Result<BoxStream<TrackEvent>>;

    async fn get_track_name(&self, id: TrackId) -> Result<String>;

    async fn set_track_name(&self, id: TrackId, name: String) -> Result<()>;

    async fn get_track_range(
        &self,
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<TrackItemId>>;

    async fn add_track_item(
        &self,
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> Result<TrackItemId>;

    async fn get_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<TrackItem>;

    async fn remove_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<()>;

    async fn move_track_item(
        &self,
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    ) -> Result<()>;

    async fn resize_track_item(
        &self,
        id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum TrackEvent {
    NameChanged {
        new_name: String,
    },
    ItemAdded {
        id: TrackItemId,
        start: RealTime,
        end: RealTime,
    },
    ItemRemoved {
        id: TrackItemId,
    },
    ItemMoved {
        id: TrackItemId,
        new_start: RealTime,
    },
    ItemResized {
        id: TrackItemId,
        new_duration: RealTime,
    },
}

#[trait_variant::make(Send)]
pub trait BlobOperations {
    async fn create_internal_blob(&self, data: Vec<u8>) -> Result<BlobId>;

    async fn create_external_blob(&self, path: PathBuf) -> Result<BlobId>;
}
