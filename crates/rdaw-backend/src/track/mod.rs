mod items;
mod ops;

use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;
use rdaw_core::collections::HashSet;

pub use self::items::TrackItems;
pub use self::ops::TrackOperation;
use crate::{Hub, Object, Uuid};

#[derive(Debug, Clone)]
pub struct Track {
    uuid: Uuid,
    pub name: String,
    pub links: TrackLinks,
    pub items: TrackItems,
    pub tempo_map_id: TempoMapId,
}

impl Track {
    pub fn new(tempo_map_id: TempoMapId, name: String) -> Track {
        Track {
            uuid: Uuid::new_v4(),
            name,
            links: TrackLinks::default(),
            items: TrackItems::new(),
            tempo_map_id,
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
        self.items.trace(hub, callback);
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
