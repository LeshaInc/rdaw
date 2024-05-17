use rdaw_core::time::RealTime;

use crate::{BlobId, Object, Uuid};

slotmap::new_key_type! {
    pub struct AudioSourceId;
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub uuid: Uuid,
    pub blob: BlobId,
    pub metadata: AudioMetadata,
}

impl Object for AudioSource {
    type Id = AudioSourceId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub sample_rate: u32,
    pub duration: RealTime,
}
