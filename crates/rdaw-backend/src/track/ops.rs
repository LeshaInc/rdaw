use rdaw_api::time::Time;
use rdaw_api::track::{
    TrackEvent, TrackHierarchy, TrackHierarchyEvent, TrackId, TrackItem, TrackItemId,
    TrackOperations, TrackRequest, TrackResponse, TrackViewEvent, TrackViewId, TrackViewItem,
};
use rdaw_api::{BackendProtocol, Error, Result};
use rdaw_rpc::StreamId;
use slotmap::Key;
use tracing::instrument;

use super::Track;
use crate::object::Metadata;
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = TrackOperations)]
impl Backend {
    #[instrument(skip_all, err)]
    #[handler]
    pub fn list_tracks(&self) -> Result<Vec<TrackId>> {
        let tracks = self.hub.tracks.iter().map(|(id, _, _)| id).collect();
        Ok(tracks)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn create_track(&mut self) -> Result<TrackId> {
        let track = Track::new(String::new());
        let id = self.hub.tracks.insert(Metadata::new(), track);

        let mut id_str = format!("{:?}", id.data());
        if let Some(v) = id_str.find('v') {
            id_str.truncate(v);
        }

        self.hub.tracks[id].name = format!("Track {id_str}");

        Ok(id)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn subscribe_track(&mut self, id: TrackId) -> Result<StreamId> {
        if !self.hub.tracks.has(id) {
            return Err(Error::InvalidId);
        }

        Ok(self.subscribers.track.subscribe(id))
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn subscribe_track_hierarchy(&mut self, id: TrackId) -> Result<StreamId> {
        if !self.hub.tracks.has(id) {
            return Err(Error::InvalidId);
        }

        Ok(self.subscribers.track_hierarchy.subscribe(id))
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn subscribe_track_view(&mut self, id: TrackViewId) -> Result<StreamId> {
        if !self.hub.arrangements.has(id.arrangement_id) {
            return Err(Error::InvalidId);
        }

        if !self.hub.tracks.has(id.track_id) {
            return Err(Error::InvalidId);
        }

        Ok(self.subscribers.track_view.subscribe(id))
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_name(&self, id: TrackId) -> Result<String> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        Ok(track.name.clone())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn set_track_name(&mut self, id: TrackId, new_name: String) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.name.clone_from(&new_name);

        let event = TrackEvent::NameChanged { new_name };
        self.subscribers.track.notify(id, event);

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_children(&self, id: TrackId) -> Result<Vec<TrackId>> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        Ok(track.links.children.clone())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_hierarchy(&self, id: TrackId) -> Result<TrackHierarchy> {
        let mut hierarchy = TrackHierarchy::new(id);

        let mut stack = Vec::new();
        stack.push(id);

        while let Some(id) = stack.pop() {
            let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
            let children = track.links.children.to_vec();
            stack.extend(children.iter().copied());
            hierarchy.set_children(id, children);
        }

        Ok(hierarchy)
    }

    fn notify_track_child_change(&mut self, id: TrackId) {
        let track = &self.hub.tracks[id];
        let new_children = track.links.children.iter().copied().collect();

        let event = TrackHierarchyEvent::ChildrenChanged { id, new_children };

        for &ancestor in &track.links.ancestors {
            self.subscribers
                .track_hierarchy
                .notify(ancestor, event.clone());
        }

        self.subscribers.track_hierarchy.notify(id, event);
    }

    fn track_dfs(&mut self, root_id: TrackId, mut callback: impl FnMut(&mut Self, TrackId)) {
        self.track_dfs_inner(root_id, &mut callback)
    }

    fn track_dfs_inner(&mut self, root_id: TrackId, callback: &mut impl FnMut(&mut Self, TrackId)) {
        callback(self, root_id);

        let children = std::mem::take(&mut self.hub.tracks[root_id].links.children);

        for &child_id in &children {
            self.track_dfs_inner(child_id, callback);
        }

        self.hub.tracks[root_id].links.children = children;
    }

    fn recompute_ancestors(&mut self, root_id: TrackId) {
        self.track_dfs(root_id, |this, track_id| {
            if !this.hub.tracks.has(track_id) {
                return;
            }

            let mut ancestors = std::mem::take(&mut this.hub.tracks[track_id].links.ancestors);
            let direct_ancestors =
                std::mem::take(&mut this.hub.tracks[track_id].links.direct_ancestors);

            ancestors.clear();

            for &ancestor_id in &direct_ancestors {
                ancestors.insert(ancestor_id);

                let Some(ancestor) = this.hub.tracks.get(ancestor_id) else {
                    continue;
                };

                for &transitive_ancestor in &ancestor.links.ancestors {
                    ancestors.insert(transitive_ancestor);
                }
            }

            this.hub.tracks[track_id].links.direct_ancestors = direct_ancestors;
            this.hub.tracks[track_id].links.ancestors = ancestors;
        });
    }

    fn add_track_ancestor(&mut self, track_id: TrackId, ancestor_id: TrackId) {
        let Some(track) = self.hub.tracks.get_mut(track_id) else {
            return;
        };

        track.links.direct_ancestors.insert(ancestor_id);
        self.recompute_ancestors(track_id);
    }

    fn remove_track_ancestor(&mut self, track_id: TrackId, ancestor_id: TrackId) {
        let Some(track) = self.hub.tracks.get_mut(track_id) else {
            return;
        };

        track.links.direct_ancestors.remove(&ancestor_id);
        self.recompute_ancestors(track_id);
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn append_track_child(&mut self, parent_id: TrackId, child_id: TrackId) -> Result<()> {
        let track = self.hub.tracks.get(parent_id).ok_or(Error::InvalidId)?;
        let index = track.links.children.len();
        self.insert_track_child(parent_id, child_id, index)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn insert_track_child(
        &mut self,
        parent_id: TrackId,
        child_id: TrackId,
        index: usize,
    ) -> Result<()> {
        if !self.hub.tracks.has(parent_id) || !self.hub.tracks.has(child_id) {
            return Err(Error::InvalidId);
        }

        if parent_id == child_id {
            return Err(Error::RecursiveTrack);
        }

        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if index > parent.links.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        if parent.links.ancestors.contains(&child_id) {
            return Err(Error::RecursiveTrack);
        }

        parent.links.children.insert(index, child_id);
        self.add_track_ancestor(child_id, parent_id);
        self.notify_track_child_change(parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn move_track(
        &mut self,
        old_parent_id: TrackId,
        old_index: usize,
        new_parent_id: TrackId,
        new_index: usize,
    ) -> Result<()> {
        if old_parent_id == new_parent_id {
            self.move_track_in_parent(old_parent_id, old_index, new_index)
        } else {
            self.move_track_between_parents(old_parent_id, old_index, new_parent_id, new_index)
        }
    }

    fn move_track_in_parent(
        &mut self,
        parent_id: TrackId,
        old_index: usize,
        new_index: usize,
    ) -> Result<()> {
        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if old_index >= parent.links.children.len() || new_index >= parent.links.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        let child_id = parent.links.children.remove(old_index);
        parent.links.children.insert(new_index, child_id);

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
            .links
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

        if new_index > new_parent.links.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        if new_parent.links.ancestors.contains(&child_id) {
            return Err(Error::RecursiveTrack);
        }

        let child_id = old_parent.links.children.remove(old_index);

        new_parent.links.children.insert(new_index, child_id);

        if !old_parent.links.children.contains(&child_id) {
            self.remove_track_ancestor(child_id, old_parent_id);
        }

        self.add_track_ancestor(child_id, new_parent_id);
        self.notify_track_child_change(old_parent_id);
        self.notify_track_child_change(new_parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn remove_track_child(&mut self, parent_id: TrackId, index: usize) -> Result<()> {
        let parent = self.hub.tracks.get_mut(parent_id).ok_or(Error::InvalidId)?;

        if index >= parent.links.children.len() {
            return Err(Error::IndexOutOfBounds);
        }

        let child_id = parent.links.children.remove(index);

        if !parent.links.children.contains(&child_id) {
            self.remove_track_ancestor(child_id, parent_id);
        }

        self.notify_track_child_change(parent_id);

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn add_track_item(&mut self, track_id: TrackId, item: TrackItem) -> Result<TrackItemId> {
        let track = self.hub.tracks.get_mut(track_id).ok_or(Error::InvalidId)?;
        let item_id = track.items.insert(item);

        for (view_id, view) in self.track_view_cache.iter_mut(track_id) {
            let arrangement = &self.hub.arrangements[view_id.arrangement_id];
            let tempo_map = &self.hub.tempo_maps[arrangement.tempo_map_id];
            let view_item = view.add_item(tempo_map, item_id, item);
            let event = TrackViewEvent::ItemAdded {
                id: item_id,
                item: view_item,
            };
            self.subscribers.track_view.notify(view_id, event);
        }

        Ok(item_id)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_item(&self, track_id: TrackId, item_id: TrackItemId) -> Result<TrackItem> {
        let track = self.hub.tracks.get(track_id).ok_or(Error::InvalidId)?;
        track.items.get(item_id).copied().ok_or(Error::InvalidId)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn remove_track_item(&mut self, track_id: TrackId, item_id: TrackItemId) -> Result<()> {
        let track = self.hub.tracks.get_mut(track_id).ok_or(Error::InvalidId)?;
        track.items.remove(item_id);

        for (view_id, view) in self.track_view_cache.iter_mut(track_id) {
            view.remove_item(item_id);
            let event = TrackViewEvent::ItemRemoved { id: item_id };
            self.subscribers.track_view.notify(view_id, event);
        }

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn move_track_item(
        &mut self,
        track_id: TrackId,
        item_id: TrackItemId,
        new_start: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(track_id).ok_or(Error::InvalidId)?;
        let item = track.items.get_mut(item_id).ok_or(Error::InvalidId)?;

        item.start = new_start;

        for (view_id, view) in self.track_view_cache.iter_mut(track_id) {
            let arrangement = &self.hub.arrangements[view_id.arrangement_id];
            let tempo_map = &self.hub.tempo_maps[arrangement.tempo_map_id];
            let new_real_start = view.move_item(tempo_map, item_id, new_start);
            let event = TrackViewEvent::ItemMoved {
                id: item_id,
                new_start,
                new_real_start,
            };
            self.subscribers.track_view.notify(view_id, event);
        }

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn resize_track_item(
        &mut self,
        track_id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(track_id).ok_or(Error::InvalidId)?;
        let item = track.items.get_mut(item_id).ok_or(Error::InvalidId)?;

        item.duration = new_duration;

        for (view_id, view) in self.track_view_cache.iter_mut(track_id) {
            let arrangement = &self.hub.arrangements[view_id.arrangement_id];
            let tempo_map = &self.hub.tempo_maps[arrangement.tempo_map_id];
            let new_real_duration = view.resize_item(tempo_map, item_id, new_duration);
            let event = TrackViewEvent::ItemResized {
                id: item_id,
                new_duration,
                new_real_duration,
            };
            self.subscribers.track_view.notify(view_id, event);
        }

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_view_item(
        &mut self,
        view_id: TrackViewId,
        item_id: TrackItemId,
    ) -> Result<TrackViewItem> {
        if !self.hub.arrangements.has(view_id.arrangement_id)
            || !self.hub.tracks.has(view_id.track_id)
        {
            return Err(Error::InvalidId);
        }

        let view = self.track_view_cache.get_or_insert(&self.hub, view_id);
        view.get_item(item_id).copied().ok_or(Error::InvalidId)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn get_track_view_range(
        &mut self,
        view_id: TrackViewId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<(TrackItemId, TrackViewItem)>> {
        if !self.hub.arrangements.has(view_id.arrangement_id)
            || !self.hub.tracks.has(view_id.track_id)
        {
            return Err(Error::InvalidId);
        }

        let arrangement = &self.hub.arrangements[view_id.arrangement_id];
        let tempo_map = &self.hub.tempo_maps[arrangement.tempo_map_id];
        let view = self.track_view_cache.get_or_insert(&self.hub, view_id);
        let range = view
            .get_range(tempo_map, start, end)
            .map(|(id, v)| (id, *v))
            .collect();
        Ok(range)
    }
}
