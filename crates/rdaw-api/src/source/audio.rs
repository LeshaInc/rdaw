use crate::blob::BlobId;
use crate::{BoxStream, Result};

slotmap::new_key_type! {
    pub struct AudioSourceId;
}

#[trait_variant::make(Send)]
pub trait AudioSourceOperations {
    async fn list_audio_sources(&self) -> Result<Vec<AudioSourceId>>;

    async fn create_audio_source(&self, blob: BlobId) -> Result<AudioSourceId>;

    async fn subscribe_audio_source(
        &self,
        id: AudioSourceId,
    ) -> Result<BoxStream<AudioSourceEvent>>;

    async fn get_audio_source_name(&self, id: AudioSourceId) -> Result<String>;

    async fn set_audio_source_name(&self, id: AudioSourceId, new_name: String) -> Result<()>;
}

#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AudioSourceEvent {
    NameChanged { new_name: String },
}
