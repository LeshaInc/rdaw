use rdaw_api::item::AudioItemId;
use rdaw_api::source::AudioSourceId;

use crate::document;
use crate::object::{DeserializationContext, Hub, Object, ObjectId, SerializationContext};

impl ObjectId for AudioItemId {
    type Object = AudioItem;
}

#[derive(Debug, Clone)]
pub struct AudioItem {
    pub source_id: AudioSourceId,
}

impl Object for AudioItem {
    type Id = AudioItemId;

    fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        if let Some(source) = hub.audio_sources.get(self.source_id) {
            callback(source);
        }
    }

    fn serialize(&self, _ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        todo!()
    }

    fn deserialize(_ctx: &DeserializationContext<'_>, _data: &[u8]) -> Result<Self, document::Error>
    where
        Self: Sized,
    {
        todo!()
    }
}
