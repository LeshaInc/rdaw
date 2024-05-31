use rdaw_api::{AudioItemId, AudioSourceId};

use crate::{Hub, Object, Uuid};

#[derive(Debug, Clone)]
pub struct AudioItem {
    pub uuid: Uuid,
    pub source: AudioSourceId,
}

impl Object for AudioItem {
    type Id = AudioItemId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        callback(self.uuid);

        if let Some(source) = hub.audio_sources.get(self.source) {
            source.trace(hub, callback);
        }
    }
}
