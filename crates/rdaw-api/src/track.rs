use rdaw_core::collections::{HashMap, ImVec};
use rdaw_core::time::RealTime;

use crate::item::ItemId;
use crate::time::Time;
use crate::{BoxStream, Result};

slotmap::new_key_type! {
    pub struct TrackId;

    pub struct TrackItemId;
}

#[trait_variant::make(Send)]
pub trait TrackOperations {
    async fn list_tracks(&self) -> Result<Vec<TrackId>>;

    async fn create_track(&self) -> Result<TrackId>;

    async fn subscribe_track(&self, id: TrackId) -> Result<BoxStream<TrackEvent>>;

    async fn subscribe_track_hierarchy(
        &self,
        root: TrackId,
    ) -> Result<BoxStream<TrackHierarchyEvent>>;

    async fn get_track_name(&self, id: TrackId) -> Result<String>;

    async fn set_track_name(&self, id: TrackId, name: String) -> Result<()>;

    async fn get_track_children(&self, parent: TrackId) -> Result<Vec<TrackId>>;

    async fn get_track_hierarchy(&self, root: TrackId) -> Result<TrackHierarchy>;

    async fn append_track_child(&self, parent: TrackId, child: TrackId) -> Result<()>;

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

#[derive(Debug, Clone)]
pub struct TrackHierarchy {
    root: TrackId,
    children: HashMap<TrackId, Vec<TrackId>>,
}

impl TrackHierarchy {
    pub fn new(root: TrackId) -> TrackHierarchy {
        TrackHierarchy {
            root,
            children: HashMap::default(),
        }
    }

    pub fn root(&self) -> TrackId {
        self.root
    }

    pub fn dfs(&self, root: TrackId, mut callback: impl FnMut(TrackNode)) {
        self.dfs_inner(
            TrackNode {
                id: root,
                index: 0,
                level: 0,
                parent: None,
            },
            &mut callback,
        );
    }

    fn dfs_inner(&self, node: TrackNode, callback: &mut impl FnMut(TrackNode)) {
        callback(node);

        let Some(children) = self.children.get(&node.id) else {
            return;
        };

        for (index, &id) in children.iter().enumerate() {
            self.dfs_inner(
                TrackNode {
                    id,
                    index,
                    level: node.level + 1,
                    parent: Some(node.id),
                },
                callback,
            );
        }
    }

    pub fn children(&self, id: TrackId) -> impl Iterator<Item = TrackId> + '_ {
        self.children.get(&id).into_iter().flatten().copied()
    }

    pub fn set_children(&mut self, id: TrackId, new_children: Vec<TrackId>) {
        self.children.insert(id, new_children);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TrackNode {
    pub id: TrackId,
    pub index: usize,
    pub level: usize,
    pub parent: Option<TrackId>,
}

#[derive(Debug, Clone)]
pub enum TrackHierarchyEvent {
    ChildrenChanged {
        id: TrackId,
        new_children: ImVec<TrackId>,
    },
}
