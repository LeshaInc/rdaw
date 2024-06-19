use std::io::{Read, Seek};

use crate::audio::AudioInputStream;
use crate::Result;

pub trait MediaInput {
    fn open<R: Read + Seek>(reader: R) -> Result<Self>
    where
        Self: Sized;

    fn get_audio_stream(&mut self) -> Result<Option<Box<dyn AudioInputStream<'_>>>>;
}
