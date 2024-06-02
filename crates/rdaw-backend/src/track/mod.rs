mod ops;
mod view;

use rdaw_api::item::ItemId;
use rdaw_api::track::{TrackId, TrackItem, TrackItemId};
use rdaw_core::collections::HashSet;
use slotmap::SlotMap;

pub use self::ops::TrackOperation;
pub use self::view::{TrackView, TrackViewCache};
use crate::{Hub, Object, Uuid};

#[derive(Debug, Clone)]
pub struct Track {
    uuid: Uuid,
    pub name: String,
    pub links: TrackLinks,
    pub items: SlotMap<TrackItemId, TrackItem>,
}

impl Track {
    pub fn new(name: String) -> Track {
        Track {
            uuid: Uuid::new_v4(),
            name,
            links: TrackLinks::default(),
            items: SlotMap::default(),
        }
    }
}

impl Object for Track {
    type Id = TrackId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        callback(self.uuid);

        self.links.trace(hub, callback);

        for item in self.items.values() {
            match item.inner {
                ItemId::Audio(id) => {
                    if let Some(item) = hub.audio_items.get(id) {
                        item.trace(hub, callback);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackLinks {
    pub children: Vec<TrackId>,
    pub ancestors: HashSet<TrackId>,
    pub direct_ancestors: HashSet<TrackId>,
}

impl TrackLinks {
    pub fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        for &child_id in &self.children {
            if let Some(child) = hub.tracks.get(child_id) {
                child.trace(hub, callback);
            }
        }
    }
}
