use rdaw_api::asset::AssetId;
use rdaw_api::audio::AudioMetadata;
use rdaw_api::source::{
    AudioSourceId, AudioSourceOperations, AudioSourceRequest, AudioSourceResponse,
};
use rdaw_api::{BackendProtocol, Result};
use rdaw_rpc::StreamId;
use tracing::instrument;

use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = AudioSourceOperations)]
impl Backend {
    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn create_audio_source(&mut self, asset_id: AssetId) -> Result<AudioSourceId> {
        let _ = asset_id;
        todo!()
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn subscribe_audio_source_name(&mut self, id: AudioSourceId) -> Result<StreamId> {
        let _ = id;
        todo!()
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_audio_source_name(&self, id: AudioSourceId) -> Result<String> {
        let _ = id;
        todo!()
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn set_audio_source_name(&mut self, id: AudioSourceId, new_name: String) -> Result<()> {
        let _ = (id, new_name);
        todo!()
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_audio_source_metadata(&self, id: AudioSourceId) -> Result<AudioMetadata> {
        let _ = id;
        todo!()
    }
}
