use std::path::PathBuf;

use futures_lite::Stream;
use rdaw_core::time::RealTime;
use rdaw_object::{BlobId, ItemId, Time, TrackId, TrackItem, TrackItemId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
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

#[trait_variant::make(Send)]
pub trait TrackOperations {
    async fn create_track(&mut self, name: String) -> Result<TrackId>;

    async fn subscribe_track(&mut self, id: TrackId) -> Result<impl Stream<Item = TrackEvent>>;

    async fn get_track_name(&self, id: TrackId) -> Result<String>;

    async fn set_track_name(&mut self, id: TrackId, name: String) -> Result<()>;

    async fn get_track_range(
        &self,
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<TrackItemId>>;

    async fn add_track_item(
        &mut self,
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> Result<TrackItemId>;

    async fn get_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<TrackItem>;

    async fn remove_track_item(&mut self, id: TrackId, item_id: TrackItemId) -> Result<()>;

    async fn move_track_item(
        &mut self,
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    ) -> Result<()>;

    async fn resize_track_item(
        &mut self,
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
    async fn create_internal_blob(&mut self, data: Vec<u8>) -> Result<BlobId>;

    async fn create_external_blob(&mut self, path: PathBuf) -> Result<BlobId>;
}
