use rdaw_core::collections::ImVec;
use rdaw_core::time::RealTime;

use crate::{BoxStream, ItemId, Result, Time};

slotmap::new_key_type! {
    pub struct TrackId;

    pub struct TrackItemId;
}

#[trait_variant::make(Send)]
pub trait TrackOperations {
    async fn list_tracks(&self) -> Result<Vec<TrackId>>;

    async fn create_track(&self) -> Result<TrackId>;

    async fn subscribe_track(&self, id: TrackId) -> Result<BoxStream<TrackEvent>>;

    async fn get_track_name(&self, id: TrackId) -> Result<String>;

    async fn set_track_name(&self, id: TrackId, name: String) -> Result<()>;

    async fn get_track_children(&self, parent: TrackId) -> Result<ImVec<TrackId>>;

    async fn insert_track_child(&self, parent: TrackId, child: TrackId, index: usize)
        -> Result<()>;

    async fn move_track(
        &self,
        old_parent: TrackId,
        old_index: usize,
        new_parent: TrackId,
        new_index: usize,
    ) -> Result<()>;

    async fn remove_track_child(&self, parent: TrackId, index: usize) -> Result<()>;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackItem {
    pub inner: ItemId,
    pub position: Time,
    pub duration: Time,
    pub real_start: RealTime,
    pub real_end: RealTime,
}

impl TrackItem {
    pub fn real_duration(&self) -> RealTime {
        self.real_end - self.real_start
    }
}

#[derive(Debug, Clone)]
pub enum TrackEvent {
    NameChanged {
        new_name: String,
    },
    ChildrenChanged {
        new_children: ImVec<TrackId>,
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
