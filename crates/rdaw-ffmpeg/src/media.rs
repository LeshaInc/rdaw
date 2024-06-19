use std::io::{Read, Seek};

use ffmpeg_sys_next as ffi;
use rdaw_api::audio::AudioMetadata;
use rdaw_api::Result;

use crate::resample::{Resampler, ResamplerConfig};
use crate::{Decoder, ErrorKind, Frame, InputContext, Packet, StreamIdx};

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

        let raw_metadata = self.context.get_audio_stream_raw_metadata(stream_idx)?;
        let metadata = self.context.get_audio_stream_metadata(stream_idx)?;

        let target_format = ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT;

        let resampler = if raw_metadata.sample_format == target_format {
            None
        } else {
            Some(Resampler::new(ResamplerConfig {
                out_ch_layout: raw_metadata.ch_layout,
                out_sample_fmt: ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                out_sample_rate: raw_metadata.sample_rate,
                in_ch_layout: raw_metadata.ch_layout,
                in_sample_fmt: raw_metadata.sample_format,
                in_sample_rate: raw_metadata.sample_rate,
            })?)
        };

        let packet = Packet::new()?;

        let frame = Frame::new()?;
        let resampler_frame = if resampler.is_some() {
            Some(Frame::new()?)
        } else {
            None
        };

        Ok(Some(Box::new(AudioInputStream {
            metadata,
            media: self,
            stream_idx,
            decoder,
            resampler,
            packet,
            frame,
            resampler_frame,
        })))
    }
}

#[allow(dead_code)]
pub struct AudioInputStream<'media, R> {
    media: &'media mut MediaInput<R>,
    metadata: AudioMetadata,
    stream_idx: StreamIdx,
    decoder: Decoder,
    resampler: Option<Resampler>,
    packet: Packet,
    frame: Frame,
    resampler_frame: Option<Frame>,
}

impl<'media, R: Read + Seek> rdaw_api::audio::AudioInputStream<'media>
    for AudioInputStream<'media, R>
{
    fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }

    fn next_frame(&mut self) -> Result<&[f32]> {
        let frame = loop {
            let packet = match self.media.context.read_packet(&mut self.packet) {
                Ok(v) => v,
                Err(e) if e.kind() == ErrorKind::Eof => return Ok(&[]),
                Err(e) => return Err(e.into()),
            };

            if packet.stream_idx() != self.stream_idx {
                continue;
            }

            self.decoder.send_packet(packet)?;

            let frame = match self.decoder.recv_frame(&mut self.frame) {
                Ok(v) => v,
                Err(e) if e.kind() == ErrorKind::Eof => return Ok(&[]),
                Err(e) if e.kind() == ErrorKind::Again => continue,
                Err(e) => return Err(e.into()),
            };

            let Some(resampler) = self.resampler.as_mut() else {
                break frame;
            };

            let resampler_frame = self.resampler_frame.as_mut().unwrap();
            let frame = resampler.convert(frame, resampler_frame)?;
            break frame;
        };

        let samples = unsafe { frame.get_f32_samples() };

        // evil lifetime extension due to borrowck restrictions
        Ok(unsafe { std::slice::from_raw_parts(samples.as_ptr(), samples.len()) })
    }
}
