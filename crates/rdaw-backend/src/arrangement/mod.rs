mod encoding;
mod ops;

use rdaw_api::arrangement::ArrangementId;
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;
use rdaw_api::Result;

use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for ArrangementId {
    type Object = Arrangement;
}

#[derive(Debug, Clone)]
pub struct Arrangement {
    pub tempo_map_id: TempoMapId,
    pub main_track_id: TrackId,
    pub name: String,
}

impl Object for Arrangement {
    type Id = ArrangementId;

    const TYPE: ObjectType = ObjectType::Arrangement;

    fn serialize(&self, ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>> {
        self::encoding::serialize(ctx, self)
    }

    fn deserialize(ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Self> {
        self::encoding::deserialize(ctx, data)
    }
}
