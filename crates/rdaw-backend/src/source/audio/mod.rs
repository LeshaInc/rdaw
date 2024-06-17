mod ops;

use rdaw_api::asset::AssetId;
use rdaw_api::audio::AudioMetadata;
use rdaw_api::source::AudioSourceId;
use rdaw_api::Result;

use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for AudioSourceId {
    type Object = AudioSource;
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub asset_id: AssetId,
    pub metadata: AudioMetadata,
}

impl Object for AudioSource {
    type Id = AudioSourceId;

    const TYPE: ObjectType = ObjectType::AudioSource;

    fn serialize(&self, _ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>> {
        todo!()
    }

    fn deserialize(_ctx: &mut DeserializationContext<'_>, _data: &[u8]) -> Result<Self> {
        todo!()
    }
}
