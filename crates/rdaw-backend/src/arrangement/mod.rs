mod encoding;
mod ops;

use rdaw_api::arrangement::ArrangementId;
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;

use crate::document;
use crate::object::{DeserializationContext, Hub, Object, SerializationContext};

#[derive(Debug, Clone)]
pub struct Arrangement {
    pub tempo_map_id: TempoMapId,
    pub main_track_id: TrackId,
    pub name: String,
}

impl Object for Arrangement {
    type Id = ArrangementId;

    fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        if let Some(tempo_map) = hub.tempo_maps.get(self.tempo_map_id) {
            callback(tempo_map);
        }

        if let Some(main_track) = hub.tracks.get(self.main_track_id) {
            callback(main_track);
        }
    }

    fn serialize(&self, ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        self::encoding::serialize(ctx, self)
    }

    fn deserialize(ctx: &DeserializationContext<'_>, data: &[u8]) -> Result<Self, document::Error>
    where
        Self: Sized,
    {
        self::encoding::deserialize(ctx, data)
    }
}
