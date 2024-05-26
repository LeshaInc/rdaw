use rdaw_api::{
    BoxStream, Error, ItemId, Result, Time, TrackEvent, TrackHierarchy, TrackHierarchyEvent,
    TrackId, TrackItem, TrackItemId, TrackOperations,
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

    GetTrackHierarchy => get_track_hierarchy(
        root: TrackId
    ) -> Result<TrackHierarchy>;

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
    pub fn get_track_children(&self, parent: TrackId) -> Result<ImVec<TrackId>> {
        let track = self.hub.tracks.get(parent).ok_or(Error::InvalidId)?;
        let children = track.children.iter().copied().collect();
        Ok(children)
    }

    #[instrument(skip_all, err)]
    pub fn get_track_hierarchy(&self, root: TrackId) -> Result<TrackHierarchy> {
        let mut hierarchy = TrackHierarchy::new(root);

        let mut stack = Vec::new();
        stack.push(root);

        while let Some(id) = stack.pop() {
            let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
            let children = track.children.to_vec();
            stack.extend(children.iter().copied());
            hierarchy.set_children(id, children);
        }

        Ok(hierarchy)
    }

    fn notify_track_child_change(&mut self, id: TrackId) {
        let track = &self.hub.tracks[id];
        let new_children = track.children.iter().copied().collect();

        let event = TrackHierarchyEvent::ChildrenChanged { id, new_children };

        for &ancestor in &track.ancestors {
            self.track_hierarchy_subscribers
                .notify(ancestor, event.clone());
        }

        self.track_hierarchy_subscribers.notify(id, event);
    }

    fn add_track_ancestor(&mut self, track_id: TrackId, ancestor_id: TrackId) {
        let Some([track, ancestor]) = self.hub.tracks.get_disjoint_mut([track_id, ancestor_id])
        else {
            return;
        };

        track.direct_ancestors.insert(ancestor_id);
        track.ancestors.insert(ancestor_id);

        for &transitive_ancestor_id in &ancestor.ancestors {
            track.ancestors.insert(transitive_ancestor_id);
        }
    }

    fn remove_track_ancestor(&mut self, track_id: TrackId, ancestor_id: TrackId) {
        let Some(track) = self.hub.tracks.get_mut(track_id) else {
            return;
        };

        track.direct_ancestors.remove(&ancestor_id);

        let mut new_ancestors = std::mem::take(&mut track.ancestors);
        new_ancestors.clear();

        for &ancestor_id in &self.hub.tracks[track_id].direct_ancestors {
            new_ancestors.insert(ancestor_id);

            let Some(ancestor) = self.hub.tracks.get(ancestor_id) else {
                continue;
            };

            for &transitive_ancestor in &ancestor.ancestors {
                new_ancestors.insert(transitive_ancestor);
            }
        }

        self.hub.tracks[track_id].ancestors = new_ancestors;
    }

    #[instrument(skip_all, err)]
    pub fn append_track_child(&mut self, parent: TrackId, child: TrackId) -> Result<()> {
        let track = self.hub.tracks.get(parent).ok_or(Error::InvalidId)?;
        let index = track.children.len();
        self.insert_track_child(parent, child, index)
    }

    #[instrument(skip_all, err)]
    pub fn insert_track_child(
        &mut self,
        parent_id: TrackId,
        child_id: TrackId,
        index: usize,
    ) -> Result<()> {
        if parent_id == child_id {
            return Err(Error::RecursiveTrack);
        }

        if !self.hub.tracks.contains_id(child_id) {
            return Err(Error::InvalidId);
        }

        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if index > parent.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        if parent.ancestors.contains(&child_id) {
            return Err(Error::RecursiveTrack);
        }

        parent.children.insert(index, child_id);
        self.add_track_ancestor(child_id, parent_id);
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

        if old_index >= parent.children.len() || new_index >= parent.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        let child_id = parent.children.remove(old_index);
        parent.children.insert(new_index, child_id);

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
        let old_parent = self.hub.tracks.get(old_parent_id).ok_or(Error::InvalidId)?;
        let &child_id = old_parent
            .children
            .get(old_index)
            .ok_or(Error::IndexOutOfBounds)?;

        if child_id == old_parent_id || child_id == new_parent_id {
            return Err(Error::RecursiveTrack);
        }

        let [old_parent, new_parent] = self
            .hub
            .tracks
            .get_disjoint_mut([old_parent_id, new_parent_id])
            .ok_or(Error::InvalidId)?;

        if new_index > new_parent.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        if new_parent.ancestors.contains(&child_id) {
            tracing::error!("HERE");
            return Err(Error::RecursiveTrack);
        }

        let child_id = old_parent.children.remove(old_index);

        new_parent.children.insert(new_index, child_id);

        if !old_parent.children.contains(&child_id) {
            self.remove_track_ancestor(child_id, old_parent_id);
        }

        self.add_track_ancestor(child_id, new_parent_id);
        self.notify_track_child_change(old_parent_id);
        self.notify_track_child_change(new_parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn remove_track_child(&mut self, parent_id: TrackId, index: usize) -> Result<()> {
        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if index >= parent.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        let child_id = parent.children.remove(index);

        if !parent.children.contains(&child_id) {
            self.remove_track_ancestor(child_id, parent_id);
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
