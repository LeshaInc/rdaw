use rdaw_api::audio::AudioMetadata;
use rdaw_api::blob::BlobId;
use rdaw_api::source::{AudioSourceEvent, AudioSourceId, AudioSourceOperations};
use rdaw_api::{BoxStream, Result};

use crate::{Backend, BackendHandle};

crate::dispatch::define_dispatch_ops! {
    pub enum AudioSourceOperation;

    impl Backend {
        pub fn dispatch_audio_source_operation;
    }

    impl AudioSourceOperations for BackendHandle;

    ListAudioSources => list_audio_sources() -> Result<Vec<AudioSourceId>>;

    CreateAudioSource => create_audio_source(
        blob: BlobId,
    ) -> Result<AudioSourceId>;

    SubscribeAudioSource => subscribe_audio_source(
        id: AudioSourceId,
    ) -> Result<BoxStream<AudioSourceEvent>>;

    GetAudioSourceName => get_audio_source_name(
        id: AudioSourceId,
    ) -> Result<String>;

    SetAudioSourceName => set_audio_source_name(
        id: AudioSourceId,
        new_name: String,
    ) -> Result<()>;

    GetAudioSourceMetadata => get_audio_source_metadata(
        id: AudioSourceId,
    ) -> Result<AudioMetadata>;
}

impl Backend {
    pub fn list_audio_sources(&self) -> Result<Vec<AudioSourceId>> {
        todo!()
    }

    pub fn create_audio_source(&mut self, _blob: BlobId) -> Result<AudioSourceId> {
        todo!()
    }

    pub fn subscribe_audio_source(
        &mut self,
        _id: AudioSourceId,
    ) -> Result<BoxStream<AudioSourceEvent>> {
        todo!()
    }

    pub fn get_audio_source_name(&self, _id: AudioSourceId) -> Result<String> {
        todo!()
    }

    pub fn set_audio_source_name(&mut self, _id: AudioSourceId, _new_name: String) -> Result<()> {
        todo!()
    }

    pub fn get_audio_source_metadata(&self, _id: AudioSourceId) -> Result<AudioMetadata> {
        todo!()
    }
}
