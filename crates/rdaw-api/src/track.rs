use rdaw_core::collections::{HashMap, ImVec};
use rdaw_core::time::RealTime;

use crate::arrangement::ArrangementId;
use crate::item::ItemId;
use crate::time::Time;
use crate::{BoxStream, Result};

slotmap::new_key_type! {
    pub struct TrackId;

    pub struct TrackItemId;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TrackViewId {
    pub track_id: TrackId,
    pub arrangement_id: ArrangementId,
}

#[rdaw_macros::api_operations]
pub trait TrackOperations {
    async fn list_tracks(&self) -> Result<Vec<TrackId>>;

    async fn create_track(&self) -> Result<TrackId>;

    #[sub]
    async fn subscribe_track(&self, id: TrackId) -> Result<BoxStream<TrackEvent>>;

    #[sub]
    async fn subscribe_track_hierarchy(
        &self,
        root: TrackId,
    ) -> Result<BoxStream<TrackHierarchyEvent>>;

    #[sub]
    async fn subscribe_track_view(&self, view_id: TrackViewId)
        -> Result<BoxStream<TrackViewEvent>>;

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

    async fn add_track_item(&self, track_id: TrackId, item: TrackItem) -> Result<TrackItemId>;

    async fn get_track_item(&self, track_id: TrackId, item_id: TrackItemId) -> Result<TrackItem>;

    async fn remove_track_item(&self, track_id: TrackId, item_id: TrackItemId) -> Result<()>;

    async fn move_track_item(
        &self,
        track_id: TrackId,
        item_id: TrackItemId,
        new_start: Time,
    ) -> Result<()>;

    async fn resize_track_item(
        &self,
        track_id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()>;

    async fn get_track_view_item(
        &self,
        view_id: TrackViewId,
        item_id: TrackItemId,
    ) -> Result<TrackViewItem>;

    async fn get_track_view_range(
        &self,
        view_id: TrackViewId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<(TrackItemId, TrackViewItem)>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackItem {
    pub inner: ItemId,
    pub start: Time,
    pub duration: Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackViewItem {
    pub inner: ItemId,
    pub start: Time,
    pub duration: Time,
    pub real_start: RealTime,
    pub real_end: RealTime,
}

impl TrackViewItem {
    pub fn real_duration(&self) -> RealTime {
        self.real_end - self.real_start
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackEvent {
    NameChanged { new_name: String },
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

#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TrackHierarchyEvent {
    ChildrenChanged {
        id: TrackId,
        new_children: ImVec<TrackId>,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TrackViewEvent {
    ItemAdded {
        id: TrackItemId,
        item: TrackViewItem,
    },
    ItemRemoved {
        id: TrackItemId,
    },
    ItemMoved {
        id: TrackItemId,
        new_start: Time,
        new_real_start: RealTime,
    },
    ItemResized {
        id: TrackItemId,
        new_duration: Time,
        new_real_duration: RealTime,
    },
}
