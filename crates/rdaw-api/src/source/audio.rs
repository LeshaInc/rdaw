use crate::asset::AssetId;
use crate::audio::AudioMetadata;
use crate::{BackendProtocol, BoxStream, Result};

slotmap::new_key_type! {
    pub struct AudioSourceId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait AudioSourceOperations {
    async fn list_audio_sources(&self) -> Result<Vec<AudioSourceId>>;

    async fn create_audio_source(&self, asset_id: AssetId) -> Result<AudioSourceId>;

    #[sub]
    async fn subscribe_audio_source_name(&self, id: AudioSourceId) -> Result<BoxStream<String>>;

    async fn get_audio_source_name(&self, id: AudioSourceId) -> Result<String>;

    async fn set_audio_source_name(&self, id: AudioSourceId, new_name: String) -> Result<()>;

    async fn get_audio_source_metadata(&self, id: AudioSourceId) -> Result<AudioMetadata>;
}
