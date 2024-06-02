mod ops;

use rdaw_api::arrangement::ArrangementId;
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;

pub use self::ops::ArrangementOperation;
use crate::{Hub, Object, Uuid};

#[derive(Debug, Clone)]
pub struct Arrangement {
    uuid: Uuid,
    pub name: String,
    pub tempo_map_id: TempoMapId,
    pub main_track_id: TrackId,
}

impl Arrangement {
    pub fn new(tempo_map_id: TempoMapId, main_track_id: TrackId, name: String) -> Arrangement {
        Arrangement {
            uuid: Uuid::new_v4(),
            name,
            tempo_map_id,
            main_track_id,
        }
    }
}

impl Object for Arrangement {
    type Id = ArrangementId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        callback(self.uuid);

        if let Some(tempo_map) = hub.tempo_maps.get(self.tempo_map_id) {
            tempo_map.trace(hub, callback);
        }

        if let Some(main_track) = hub.tracks.get(self.main_track_id) {
            main_track.trace(hub, callback);
        }
    }
}
