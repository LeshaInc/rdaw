use crate::audio::AudioInputStream;
use crate::Result;

pub trait OpenMediaInput<R>: Sized {
    fn open(reader: R) -> Result<Self>;
}

pub trait MediaInput {
    fn get_audio_stream(&mut self) -> Result<Option<Box<dyn AudioInputStream<'_> + '_>>>;
}
