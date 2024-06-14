mod ops;

use rdaw_api::audio::AudioMetadata;
use rdaw_api::blob::BlobId;
use rdaw_api::source::AudioSourceId;

use crate::document;
use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for AudioSourceId {
    type Object = AudioSource;
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub blob_id: BlobId,
    pub metadata: AudioMetadata,
}

impl Object for AudioSource {
    type Id = AudioSourceId;

    const TYPE: ObjectType = ObjectType::AudioSource;

    fn serialize(&self, _ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        todo!()
    }

    fn deserialize(
        _ctx: &mut DeserializationContext<'_>,
        _data: &[u8],
    ) -> Result<Self, document::Error> {
        todo!()
    }
}
