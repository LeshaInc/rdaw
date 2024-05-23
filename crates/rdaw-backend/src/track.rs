use rdaw_api::{
    BoxStream, Error, ItemId, Result, Time, TrackEvent, TrackId, TrackItem, TrackItemId,
    TrackOperations,
};
use rdaw_core::collections::ImVec;
use rdaw_object::{BeatMap, Track};
use slotmap::Key;
use tracing::instrument;

use crate::{Backend, BackendHandle};

crate::dispatch::define_dispatch_ops! {
    pub enum TrackOperation;

    impl Backend {
        pub fn dispatch_track_operation;
    }

    impl TrackOperations for BackendHandle;

    ListTracks => list_tracks() -> Result<Vec<TrackId>>;

    CreateTrack => create_track() -> Result<TrackId>;

    SubscribeTrack => subscribe_track(
        id: TrackId,
    ) -> Result<BoxStream<TrackEvent>>;

    GetTrackName => get_track_name(
        id: TrackId,
    ) -> Result<String>;

    SetTrackName => set_track_name(
        id: TrackId,
        new_name: String,
    ) -> Result<()>;

    GetTrackChildren => get_track_children(
        parent: TrackId
    ) -> Result<ImVec<TrackId>>;

    AppendTrackChild => append_track_child(
        parent: TrackId,
        child: TrackId,
    ) -> Result<()>;

    InsertTrackChild => insert_track_child(
        parent: TrackId,
        child: TrackId,
        index: usize,
    ) -> Result<()>;

    MoveTrack => move_track(
        old_parent: TrackId,
        old_index: usize,
        new_parent: TrackId,
        new_index: usize,
    ) -> Result<()>;

    RemoveTrackChild => remove_track_child(
        parent: TrackId,
        index: usize
    ) -> Result<()>;

    GetTrackRange => get_track_range(
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<TrackItemId>>;

    AddTrackItem => add_track_item(
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> Result<TrackItemId>;

    GetTrackItem => get_track_item(
        id: TrackId,
        item_id: TrackItemId,
    ) -> Result<TrackItem>;

    RemoveTrackItem => remove_track_item(
        id: TrackId,
        item_id: TrackItemId,
    ) -> Result<()>;

    MoveTrackItem => move_track_item(
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    ) -> Result<()>;

    ResizeTrackItem => resize_track_item(
        id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()>;
}

impl Backend {
    #[instrument(skip_all, err)]
    pub async fn list_tracks(&self) -> Result<Vec<TrackId>> {
        let tracks = self.hub.tracks.iter().map(|(id, _)| id).collect();
        Ok(tracks)
    }

    #[instrument(skip_all, err)]
    pub async fn create_track(&mut self) -> Result<TrackId> {
        // TODO: remove this
        let beat_map = BeatMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let track = Track::new(beat_map, String::new());
        let id = self.hub.tracks.insert(track);

        let mut id_str = format!("{:?}", id.data());
        if let Some(v) = id_str.find('v') {
            id_str.truncate(v);
        }

        self.hub.tracks[id].name = format!("Track {id_str}");

        Ok(id)
    }

    #[instrument(skip_all, err)]
    pub async fn subscribe_track(&mut self, id: TrackId) -> Result<BoxStream<TrackEvent>> {
        if !self.hub.tracks.contains_id(id) {
            return Err(Error::InvalidId);
        }

        Ok(Box::pin(self.track_subscribers.subscribe(id)))
    }

    #[instrument(skip_all, err)]
    pub async fn get_track_name(&self, id: TrackId) -> Result<String> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        Ok(track.name.clone())
    }

    #[instrument(skip_all, err)]
    pub async fn set_track_name(&mut self, id: TrackId, new_name: String) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.name.clone_from(&new_name);

        let event = TrackEvent::NameChanged { new_name };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    async fn get_track_children(&self, parent: TrackId) -> Result<ImVec<TrackId>> {
        let track = self.hub.tracks.get(parent).ok_or(Error::InvalidId)?;
        let children = track.children().collect();
        Ok(children)
    }

    #[instrument(skip_all, err)]
    async fn append_track_child(&mut self, parent: TrackId, child: TrackId) -> Result<()> {
        let track = self.hub.tracks.get_mut(parent).ok_or(Error::InvalidId)?;

        // TODO: check for recursive relationships

        track.append_child(child);

        let new_children = track.children().collect();
        let event = TrackEvent::ChildrenChanged { new_children };
        self.track_subscribers.notify(parent, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    async fn insert_track_child(
        &mut self,
        parent: TrackId,
        child: TrackId,
        index: usize,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(parent).ok_or(Error::InvalidId)?;

        if index > track.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        // TODO: check for recursive relationships

        track.insert_child(child, index);

        let new_children = track.children().collect();
        let event = TrackEvent::ChildrenChanged { new_children };
        self.track_subscribers.notify(parent, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn move_track(
        &mut self,
        old_parent: TrackId,
        old_index: usize,
        new_parent: TrackId,
        new_index: usize,
    ) -> Result<()> {
        let tracks = &mut self.hub.tracks;

        if !tracks.contains_id(old_parent) || !tracks.contains_id(new_parent) {
            return Err(Error::InvalidId);
        }

        if old_index >= tracks[old_parent].children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        if old_parent == new_parent {
            tracks[old_parent].move_child(old_index, new_index);
        } else {
            if new_index > tracks[new_parent].children().len() {
                return Err(Error::IndexOutOfBounds);
            }

            let id = tracks[old_parent].remove_child(old_index);
            tracks[new_parent].insert_child(id, new_index);
        }

        let new_children = tracks[old_parent].children().collect();
        let event = TrackEvent::ChildrenChanged { new_children };
        self.track_subscribers.notify(old_parent, event).await;

        if old_parent != new_parent {
            let new_children = tracks[new_parent].children().collect();
            let event = TrackEvent::ChildrenChanged { new_children };
            self.track_subscribers.notify(new_parent, event).await;
        }

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn remove_track_child(&mut self, parent: TrackId, index: usize) -> Result<()> {
        let track = self.hub.tracks.get_mut(parent).ok_or(Error::InvalidId)?;

        if index >= track.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        track.remove_child(index);

        let new_children = track.children().collect();
        let event = TrackEvent::ChildrenChanged { new_children };
        self.track_subscribers.notify(parent, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn get_track_range(
        &self,
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<TrackItemId>> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        let items = track.range(start, end).map(|(id, _)| id).collect();
        Ok(items)
    }

    #[instrument(skip_all, err)]
    pub async fn add_track_item(
        &mut self,
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> Result<TrackItemId> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        let item_id = track.insert(item_id, position, duration);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let event = TrackEvent::ItemAdded {
            id: item_id,
            start: item.real_start,
            end: item.real_end,
        };
        self.track_subscribers.notify(id, event).await;

        Ok(item_id)
    }

    #[instrument(skip_all, err)]
    pub async fn get_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<TrackItem> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        Ok(item.clone())
    }

    #[instrument(skip_all, err)]
    pub async fn remove_track_item(&mut self, id: TrackId, item_id: TrackItemId) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.remove(item_id);

        let event = TrackEvent::ItemRemoved { id: item_id };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn move_track_item(
        &mut self,
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.move_item(item_id, new_position);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let event = TrackEvent::ItemMoved {
            id: item_id,
            new_start: item.real_start,
        };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn resize_track_item(
        &mut self,
        id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.move_item(item_id, new_duration);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let new_duration = item.real_duration();

        let event = TrackEvent::ItemResized {
            id: item_id,
            new_duration,
        };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }
}
