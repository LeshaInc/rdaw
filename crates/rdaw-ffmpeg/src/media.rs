use std::io::{Read, Seek};

use rdaw_api::audio::AudioMetadata;
use rdaw_api::Result;

use crate::{Decoder, InputContext, StreamIdx};

pub struct MediaInput<R> {
    context: InputContext<R>,
}

impl<R: Read + Seek> rdaw_api::media::OpenMediaInput<R> for MediaInput<R> {
    fn open(reader: R) -> Result<MediaInput<R>>
    where
        Self: Sized,
    {
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

        let metadata = self.context.get_audio_stream_metadata(stream_idx)?;

        Ok(Some(Box::new(AudioInputStream {
            metadata,
            media: self,
            stream_idx,
            decoder,
        })))
    }
}

#[allow(dead_code)]
pub struct AudioInputStream<'media, R> {
    metadata: AudioMetadata,
    media: &'media mut MediaInput<R>,
    stream_idx: StreamIdx,
    decoder: Decoder,
}

impl<'media, R: Read + Seek> rdaw_api::audio::AudioInputStream<'media>
    for AudioInputStream<'media, R>
{
    fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }

    fn next_frame(&mut self, _buf: &mut [f32]) -> Result<usize> {
        todo!()
    }
}
