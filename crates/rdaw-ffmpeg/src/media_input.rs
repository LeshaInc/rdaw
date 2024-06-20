use std::io::{Read, Seek};

use rdaw_api::Result;

use crate::internal::init;
use crate::internal::input::InputContext;
use crate::AudioInputStream;

#[derive(Debug)]
pub struct MediaInput<R> {
    pub(crate) context: InputContext<R>,
}

impl<R: Read + Seek> rdaw_api::media::OpenMediaInput<R> for MediaInput<R> {
    fn open(reader: R) -> Result<MediaInput<R>>
    where
        Self: Sized,
    {
        init();
        let context = InputContext::new(reader)?;
        Ok(MediaInput { context })
    }
}

impl<R: Read + Seek> rdaw_api::media::MediaInput for MediaInput<R> {
    fn get_audio_stream(
        &mut self,
    ) -> Result<Option<Box<dyn rdaw_api::audio::AudioInputStream<'_> + '_>>> {
        let Some((stream_idx, decoder)) = self.context.find_audio_stream()? else {
            return Ok(None);
        };

        let stream = AudioInputStream::new(self, stream_idx, decoder)?;
        Ok(Some(Box::new(stream)))
    }
}
