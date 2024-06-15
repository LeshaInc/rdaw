use rdaw_api::item::AudioItemId;
use rdaw_api::source::AudioSourceId;
use rdaw_api::Result;

use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for AudioItemId {
    type Object = AudioItem;
}

#[derive(Debug, Clone)]
pub struct AudioItem {
    pub source_id: AudioSourceId,
}

impl Object for AudioItem {
    type Id = AudioItemId;

    const TYPE: ObjectType = ObjectType::AudioItem;

    fn serialize(&self, _ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>> {
        todo!()
    }

    fn deserialize(_ctx: &mut DeserializationContext<'_>, _data: &[u8]) -> Result<Self> {
        todo!()
    }
}
