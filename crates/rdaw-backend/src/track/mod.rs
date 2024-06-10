mod ops;
#[cfg(test)]
mod tests;
mod view;

use rdaw_api::item::ItemId;
use rdaw_api::track::{TrackId, TrackItem, TrackItemId};
use rdaw_core::collections::HashSet;
use slotmap::SlotMap;

pub use self::view::{TrackView, TrackViewCache};
use crate::document;
use crate::object::{DeserializationContext, Hub, Object, SerializationContext};

#[derive(Debug, Clone)]
pub struct Track {
    pub name: String,
    pub links: TrackLinks,
    pub items: SlotMap<TrackItemId, TrackItem>,
}

impl Track {
    pub fn new(name: String) -> Track {
        Track {
            name,
            links: TrackLinks::default(),
            items: SlotMap::default(),
        }
    }
}

impl Object for Track {
    type Id = TrackId;

    fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        self.links.trace(hub, callback);

        for item in self.items.values() {
            match item.inner {
                ItemId::Audio(id) => {
                    if let Some(item) = hub.audio_items.get(id) {
                        callback(item);
                    }
                }
            }
        }
    }

    fn serialize(&self, ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        let _ = ctx;
        todo!()
    }

    fn deserialize(ctx: &DeserializationContext<'_>, data: &[u8]) -> Result<Self, document::Error>
    where
        Self: Sized,
    {
        let _ = (ctx, data);
        todo!()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackLinks {
    pub children: Vec<TrackId>,
    pub ancestors: HashSet<TrackId>,
    pub direct_ancestors: HashSet<TrackId>,
}

impl TrackLinks {
    pub fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        for &child_id in &self.children {
            if let Some(child) = hub.tracks.get(child_id) {
                callback(child);
            }
        }
    }
}
