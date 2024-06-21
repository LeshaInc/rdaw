use crate::audio::AudioInputStream;
use crate::Result;

pub trait OpenMediaInput<R>: Sized {
    fn open(reader: R) -> Result<Self>;
}

pub trait MediaInput {
    type AudioInputStream<'a>: AudioInputStream<'a>
    where
        Self: 'a;

    fn get_audio_stream(&mut self) -> Result<Option<Self::AudioInputStream<'_>>>;
}
