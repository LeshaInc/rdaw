use rdaw_api::{ArrangementId, TempoMapId, TrackId};

use crate::{Hub, Object, Uuid};

#[derive(Debug, Clone)]
pub struct Arrangement {
    uuid: Uuid,
    pub tempo_map_id: TempoMapId,
    pub master_track_id: TrackId,
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

        if let Some(master_track) = hub.tracks.get(self.master_track_id) {
            master_track.trace(hub, callback);
        }
    }
}
