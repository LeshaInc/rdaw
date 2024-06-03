mod ops;

use rdaw_api::audio::AudioMetadata;
use rdaw_api::blob::BlobId;
use rdaw_api::source::AudioSourceId;

pub use self::ops::AudioSourceOperation;
use crate::{Object, Uuid};

#[derive(Debug, Clone)]
pub struct AudioSource {
    uuid: Uuid,
    blob: BlobId,
    metadata: AudioMetadata,
}

impl AudioSource {
    pub fn new(blob: BlobId, metadata: AudioMetadata) -> AudioSource {
        AudioSource {
            uuid: Uuid::new_v4(),
            blob,
            metadata,
        }
    }

    pub fn blob(&self) -> BlobId {
        self.blob
    }

    pub fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }
}

impl Object for AudioSource {
    type Id = AudioSourceId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }
}
