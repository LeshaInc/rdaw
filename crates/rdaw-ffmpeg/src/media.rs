use std::io::{Read, Seek};
use std::mem::ManuallyDrop;

use ffmpeg_sys_next as ffi;
use rdaw_api::audio::AudioMetadata;
use rdaw_api::Result;

use crate::resample::{Resampler, ResamplerConfig};
use crate::{Decoder, ErrorKind, Frame, InputContext, Packet, StreamIdx};

#[derive(Debug)]
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
                out_ch_layout: raw_metadata.channel_layout,
                out_sample_format: target_format,
                out_sample_rate: raw_metadata.sample_rate,
                in_ch_layout: raw_metadata.channel_layout,
                in_sample_format: raw_metadata.sample_format,
                in_sample_rate: raw_metadata.sample_rate,
            })?)
        };

        let packet = Packet::new()?;
        let frame = Frame::new()?;

        Ok(Some(Box::new(AudioInputStream {
            metadata,
            media: self,
            stream_idx,
            decoder,
            resampler,
            packet,
            frame,
        })))
    }
}

#[derive(Debug)]
pub struct AudioInputStream<'media, R> {
    media: &'media mut MediaInput<R>,
    metadata: AudioMetadata,
    stream_idx: StreamIdx,
    decoder: Decoder,
    resampler: Option<Resampler>,
    packet: Packet,
    frame: Frame,
}

impl<'media, R: Read + Seek> rdaw_api::audio::AudioInputStream<'media>
    for AudioInputStream<'media, R>
{
    fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }

    fn next_frame(&mut self) -> Result<&[f32]> {
        loop {
            let packet = match self.media.context.read_packet(&mut self.packet) {
                Ok(v) => Some(v),
                Err(e) if e.kind() == ErrorKind::Eof => None,
                Err(e) => return Err(e.into()),
            };

            if let Some(packet) = packet {
                if packet.stream_idx() != self.stream_idx {
                    continue;
                }

                self.decoder.send_packet(packet)?;
            } else {
                self.decoder.flush()?;
            }

            let frame = match self.decoder.recv_frame(&mut self.frame) {
                Ok(v) => Some(v),
                Err(e) if e.kind() == ErrorKind::Eof => None,
                Err(e) if e.kind() == ErrorKind::Again => continue,
                Err(e) => return Err(e.into()),
            };

            let Some(resampler) = self.resampler.as_mut() else {
                if let Some(frame) = frame {
                    let frame = ManuallyDrop::new(frame);
                    let data = unsafe { frame.get_data() };
                    return Ok(unsafe {
                        std::slice::from_raw_parts(data.as_ptr() as *const f32, data.len() / 4)
                    });
                } else {
                    return Ok(&[]);
                }
            };

            if let Some(frame) = frame {
                let frame = ManuallyDrop::new(frame);
                let data = unsafe { frame.get_data() };
                let data = resampler.convert(data)?;
                return Ok(unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const f32, data.len() / 4)
                });
            } else {
                let data = resampler.flush()?;
                return Ok(unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const f32, data.len() / 4)
                });
            }
        }
    }
}
