mod ops;

use rdaw_api::audio::AudioMetadata;
use rdaw_api::blob::BlobId;
use rdaw_api::source::AudioSourceId;

use crate::document;
use crate::object::{DeserializationContext, Hub, Object, SerializationContext};

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub blob_id: BlobId,
    pub metadata: AudioMetadata,
}

impl Object for AudioSource {
    type Id = AudioSourceId;

    fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        if let Some(blob) = hub.blobs.get(self.blob_id) {
            callback(blob);
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
