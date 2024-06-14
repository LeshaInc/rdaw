mod encoding;
mod ops;
#[cfg(test)]
mod tests;
mod view;

use rdaw_api::track::{TrackId, TrackItem, TrackItemId};
use rdaw_core::collections::HashSet;
use slotmap::SlotMap;

pub use self::view::{TrackView, TrackViewCache};
use crate::document;
use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for TrackId {
    type Object = Track;
}

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

    const TYPE: ObjectType = ObjectType::Track;

    fn serialize(&self, ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        self::encoding::serialize(ctx, self)
    }

    fn deserialize(
        ctx: &mut DeserializationContext<'_>,
        data: &[u8],
    ) -> Result<Self, document::Error> {
        self::encoding::deserialize(ctx, data)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackLinks {
    pub children: Vec<TrackId>,
    pub ancestors: HashSet<TrackId>,
    pub direct_ancestors: HashSet<TrackId>,
}
