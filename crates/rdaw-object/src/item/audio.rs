use crate::{AudioSourceId, Hub, Object, Uuid};

slotmap::new_key_type! {
    pub struct AudioItemId;
}

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
        let source = &hub.audio_sources[self.source];
        callback(source.uuid);
        source.trace(hub, callback);
    }
}
