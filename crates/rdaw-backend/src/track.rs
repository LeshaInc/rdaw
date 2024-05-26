use rdaw_api::{
    BoxStream, Error, ItemId, Result, Time, TrackEvent, TrackHierarchyEvent, TrackId, TrackItem,
    TrackItemId, TrackOperations,
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

    SubscribeTrackHierarchy => subscribe_track_hierarchy(
        root: TrackId,
    ) -> Result<BoxStream<TrackHierarchyEvent>>;

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
    pub fn list_tracks(&self) -> Result<Vec<TrackId>> {
        let tracks = self.hub.tracks.iter().map(|(id, _)| id).collect();
        Ok(tracks)
    }

    #[instrument(skip_all, err)]
    pub fn create_track(&mut self) -> Result<TrackId> {
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
    pub fn subscribe_track(&mut self, id: TrackId) -> Result<BoxStream<TrackEvent>> {
        if !self.hub.tracks.contains_id(id) {
            return Err(Error::InvalidId);
        }

        Ok(Box::pin(self.track_subscribers.subscribe(id)))
    }

    #[instrument(skip_all, err)]
    pub fn subscribe_track_hierarchy(
        &mut self,
        id: TrackId,
    ) -> Result<BoxStream<TrackHierarchyEvent>> {
        if !self.hub.tracks.contains_id(id) {
            return Err(Error::InvalidId);
        }

        Ok(Box::pin(self.track_hierarchy_subscribers.subscribe(id)))
    }

    #[instrument(skip_all, err)]
    pub fn get_track_name(&self, id: TrackId) -> Result<String> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        Ok(track.name.clone())
    }

    #[instrument(skip_all, err)]
    pub fn set_track_name(&mut self, id: TrackId, new_name: String) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.name.clone_from(&new_name);

        let event = TrackEvent::NameChanged { new_name };
        self.track_subscribers.notify(id, event);

        Ok(())
    }

    #[instrument(skip_all, err)]
    fn get_track_children(&self, parent: TrackId) -> Result<ImVec<TrackId>> {
        let track = self.hub.tracks.get(parent).ok_or(Error::InvalidId)?;
        let children = track.children().collect();
        Ok(children)
    }

    fn notify_track_child_change(&mut self, id: TrackId) {
        let track = &self.hub.tracks[id];
        let new_children = track.children().collect();

        let event = TrackHierarchyEvent::ChildrenChanged { id, new_children };

        for ancestor in track.ancestors() {
            self.track_hierarchy_subscribers
                .notify(ancestor, event.clone());
        }

        self.track_hierarchy_subscribers.notify(id, event);
    }

    #[instrument(skip_all, err)]
    fn append_track_child(&mut self, parent: TrackId, child: TrackId) -> Result<()> {
        let track = self.hub.tracks.get(parent).ok_or(Error::InvalidId)?;
        let index = track.children().len();
        self.insert_track_child(parent, child, index)
    }

    #[instrument(skip_all, err)]
    fn insert_track_child(
        &mut self,
        parent_id: TrackId,
        child_id: TrackId,
        index: usize,
    ) -> Result<()> {
        if parent_id == child_id {
            return Err(Error::RecursiveTrack);
        }

        let [parent, child] = self
            .hub
            .tracks
            .get_disjoint_mut([parent_id, child_id])
            .ok_or(Error::InvalidId)?;

        if index > parent.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        if parent.contains_ancestor(child_id) {
            return Err(Error::RecursiveTrack);
        }

        parent.insert_child(child_id, index);
        child.add_ancestor(parent_id);

        self.notify_track_child_change(parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn move_track(
        &mut self,
        old_parent: TrackId,
        old_index: usize,
        new_parent: TrackId,
        new_index: usize,
    ) -> Result<()> {
        if old_parent == new_parent {
            self.move_track_in_parent(old_parent, old_index, new_index)
        } else {
            self.move_track_between_parents(old_parent, old_index, new_parent, new_index)
        }
    }

    fn move_track_in_parent(
        &mut self,
        parent_id: TrackId,
        old_index: usize,
        new_index: usize,
    ) -> Result<()> {
        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if old_index >= parent.children().len() || new_index >= parent.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        parent.move_child(old_index, new_index);

        self.notify_track_child_change(parent_id);

        Ok(())
    }

    fn move_track_between_parents(
        &mut self,
        old_parent_id: TrackId,
        old_index: usize,
        new_parent_id: TrackId,
        new_index: usize,
    ) -> Result<()> {
        let child_id = self
            .hub
            .tracks
            .get(old_parent_id)
            .ok_or(Error::IndexOutOfBounds)?
            .get_child(old_index)
            .ok_or(Error::IndexOutOfBounds)?;

        if child_id == old_parent_id || child_id == new_parent_id {
            return Err(Error::RecursiveTrack);
        }

        let [old_parent, new_parent, child] = self
            .hub
            .tracks
            .get_disjoint_mut([old_parent_id, new_parent_id, child_id])
            .ok_or(Error::InvalidId)?;

        if new_index > new_parent.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        if new_parent.contains_ancestor(child_id) {
            return Err(Error::RecursiveTrack);
        }

        let child_id = old_parent.remove_child(old_index);

        if !old_parent.children().any(|v| v == child_id) {
            child.remove_ancestor(old_parent_id);
        }

        new_parent.insert_child(child_id, new_index);
        child.add_ancestor(new_parent_id);

        self.notify_track_child_change(old_parent_id);
        self.notify_track_child_change(new_parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn remove_track_child(&mut self, parent_id: TrackId, index: usize) -> Result<()> {
        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if index >= parent.children().len() {
            return Err(Error::IndexOutOfBounds);
        }

        let child_id = parent.remove_child(index);

        if !parent.children().any(|v| v == child_id) {
            let child = self.hub.tracks.get_mut(child_id).ok_or(Error::InvalidId)?;
            child.remove_ancestor(parent_id);
        }

        self.notify_track_child_change(parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn get_track_range(
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
    pub fn add_track_item(
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
        self.track_subscribers.notify(id, event);

        Ok(item_id)
    }

    #[instrument(skip_all, err)]
    pub fn get_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<TrackItem> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        Ok(item.clone())
    }

    #[instrument(skip_all, err)]
    pub fn remove_track_item(&mut self, id: TrackId, item_id: TrackItemId) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.remove(item_id);

        let event = TrackEvent::ItemRemoved { id: item_id };
        self.track_subscribers.notify(id, event);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn move_track_item(
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
        self.track_subscribers.notify(id, event);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn resize_track_item(
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
        self.track_subscribers.notify(id, event);

        Ok(())
    }
}
