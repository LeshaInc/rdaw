use std::io::{Read, Seek};
use std::ptr;

use ffmpeg_sys_next as ffi;
use rdaw_api::audio::{AudioChannel, AudioMetadata, SampleFormat};
use rdaw_core::time::RealTime;

use super::decoder::Decoder;
use super::error::{Error, Result};
use super::packet::{FilledPacket, Packet};
use super::reader::Reader;
use super::StreamIdx;

#[derive(Debug)]
pub struct InputContext<R> {
    _reader: Reader<R>,
    raw: *mut ffi::AVFormatContext,
}

impl<R: Read + Seek> InputContext<R> {
    pub fn new(reader: R) -> Result<InputContext<R>> {
        let mut reader = Reader::new(reader)?;

        let mut raw = unsafe { ffi::avformat_alloc_context() };
        if raw.is_null() {
            return Err(Error::new_oom("avformat"));
        }

        unsafe {
            (*raw).pb = reader.as_raw();
        }

        let res = unsafe {
            ffi::avformat_open_input(&mut raw, ptr::null(), ptr::null(), ptr::null_mut())
        };
        if res < 0 {
            return Err(Error::new(res, "avformat_open_input"));
        }

        let res = unsafe { ffi::avformat_find_stream_info(raw, ptr::null_mut()) };
        if res < 0 {
            return Err(Error::new(res, "avformat_find_stream_info"));
        }

        Ok(InputContext {
            _reader: reader,
            raw,
        })
    }

    pub fn find_audio_stream(&self) -> Result<Option<(StreamIdx, Decoder)>> {
        let mut codec = ptr::null();

        let res = unsafe {
            ffi::av_find_best_stream(
                self.raw,
                ffi::AVMediaType::AVMEDIA_TYPE_AUDIO,
                -1, // stream_nb: automatic selection
                -1, // no related stream
                &mut codec,
                0, // no flags
            )
        };

        if res < 0 {
            return Err(Error::new(res, "av_find_best_stream"));
        }

        if codec.is_null() {
            return Err(Error::new(ffi::AVERROR_BUG, "av_find_best_stream"));
        }

        let stream_idx = StreamIdx(res);

        let streams = unsafe {
            std::slice::from_raw_parts((*self.raw).streams, (*self.raw).nb_streams as usize)
        };

        let stream = unsafe { &*streams[stream_idx.0 as usize] };
        let codecpar = stream.codecpar;

        let decoder = Decoder::new(codec, codecpar)?;

        Ok(Some((stream_idx, decoder)))
    }

    pub fn get_audio_stream_raw_metadata(&self, idx: StreamIdx) -> Result<RawAudioMetadata> {
        let streams = unsafe {
            std::slice::from_raw_parts((*self.raw).streams, (*self.raw).nb_streams as usize)
        };

        let stream = streams
            .iter()
            .copied()
            .find(|&v| unsafe { (*v).index == idx.0 })
            .expect("no such stream");

        let stream = unsafe { &*stream };
        let codecpar = unsafe { &*stream.codecpar };

        Ok(RawAudioMetadata {
            channel_layout: &codecpar.ch_layout,
            sample_format: unsafe { std::mem::transmute(codecpar.format) },
            sample_rate: codecpar.sample_rate,
            duration_ns: stream.duration * (stream.time_base.num as i64) * 1_000_000_000
                / (stream.time_base.den as i64),
        })
    }

    pub fn get_audio_stream_metadata(&self, idx: StreamIdx) -> Result<AudioMetadata> {
        let raw = self.get_audio_stream_raw_metadata(idx)?;
        Ok(AudioMetadata {
            channels: convert_channel_layout(raw.channel_layout),
            sample_rate: raw.sample_rate as u32,
            sample_format: convert_sample_format(raw.sample_format),
            duration: RealTime::from_nanos(raw.duration_ns),
        })
    }

    pub fn read_packet<'a>(&mut self, packet: &'a mut Packet) -> Result<FilledPacket<'a>, Error> {
        let res = unsafe { ffi::av_read_frame(self.raw, packet.as_raw()) };
        if res < 0 {
            return Err(Error::new(res, "av_read_frame"));
        }
        Ok(unsafe { packet.assume_filled() })
    }
}

impl<R> Drop for InputContext<R> {
    fn drop(&mut self) {
        unsafe {
            ffi::avformat_close_input(&mut self.raw);
        }
    }
}

pub struct RawAudioMetadata<'a> {
    pub channel_layout: &'a ffi::AVChannelLayout,
    pub sample_format: ffi::AVSampleFormat,
    pub sample_rate: i32,
    pub duration_ns: i64,
}

#[rustfmt::skip]
const CHANNEL_MAPPING: [(ffi::AVChannel, AudioChannel); 30] = [
    (ffi::AVChannel::AV_CHAN_FRONT_LEFT, AudioChannel::FrontLeft),
    (ffi::AVChannel::AV_CHAN_FRONT_RIGHT, AudioChannel::FrontRight),
    (ffi::AVChannel::AV_CHAN_FRONT_CENTER, AudioChannel::FrontCenter),
    (ffi::AVChannel::AV_CHAN_LOW_FREQUENCY, AudioChannel::LowFrequency),
    (ffi::AVChannel::AV_CHAN_BACK_LEFT, AudioChannel::RearLeft),
    (ffi::AVChannel::AV_CHAN_BACK_RIGHT, AudioChannel::RearRight),
    (ffi::AVChannel::AV_CHAN_FRONT_LEFT_OF_CENTER, AudioChannel::FrontLeftCenter),
    (ffi::AVChannel::AV_CHAN_FRONT_RIGHT_OF_CENTER, AudioChannel::FrontRightCenter),
    (ffi::AVChannel::AV_CHAN_BACK_CENTER, AudioChannel::RearCenter),
    (ffi::AVChannel::AV_CHAN_SIDE_LEFT, AudioChannel::SideLeft),
    (ffi::AVChannel::AV_CHAN_SIDE_RIGHT, AudioChannel::SideRight),
    (ffi::AVChannel::AV_CHAN_TOP_CENTER, AudioChannel::TopCenter),
    (ffi::AVChannel::AV_CHAN_TOP_FRONT_LEFT, AudioChannel::TopFrontLeft),
    (ffi::AVChannel::AV_CHAN_TOP_FRONT_CENTER, AudioChannel::TopFrontCenter),
    (ffi::AVChannel::AV_CHAN_TOP_FRONT_RIGHT, AudioChannel::TopFrontRight),
    (ffi::AVChannel::AV_CHAN_TOP_BACK_LEFT, AudioChannel::TopRearLeft),
    (ffi::AVChannel::AV_CHAN_TOP_BACK_CENTER, AudioChannel::TopRearCenter),
    (ffi::AVChannel::AV_CHAN_TOP_BACK_RIGHT, AudioChannel::TopRearRight),
    (ffi::AVChannel::AV_CHAN_WIDE_LEFT, AudioChannel::FrontLeftWide),
    (ffi::AVChannel::AV_CHAN_WIDE_RIGHT, AudioChannel::FrontRightWide),
    (ffi::AVChannel::AV_CHAN_LOW_FREQUENCY_2, AudioChannel::LowFrequency2),
    (ffi::AVChannel::AV_CHAN_TOP_SIDE_LEFT, AudioChannel::TopSideLeft),
    (ffi::AVChannel::AV_CHAN_TOP_SIDE_RIGHT, AudioChannel::TopSideRight),
    (ffi::AVChannel::AV_CHAN_BOTTOM_FRONT_CENTER, AudioChannel::BottomCenter),
    (ffi::AVChannel::AV_CHAN_BOTTOM_FRONT_LEFT, AudioChannel::BottomLeftCenter),
    (ffi::AVChannel::AV_CHAN_BOTTOM_FRONT_RIGHT, AudioChannel::BottomRightCenter),

    // FIXME: the following channels don't have a clear analog
    (ffi::AVChannel::AV_CHAN_STEREO_LEFT, AudioChannel::FrontLeft),
    (ffi::AVChannel::AV_CHAN_STEREO_RIGHT, AudioChannel::FrontRight),
    (ffi::AVChannel::AV_CHAN_SURROUND_DIRECT_LEFT, AudioChannel::FrontLeft),
    (ffi::AVChannel::AV_CHAN_SURROUND_DIRECT_RIGHT, AudioChannel::FrontRight),
];

fn convert_channel_layout(layout: &ffi::AVChannelLayout) -> Vec<AudioChannel> {
    if layout.nb_channels <= 0 {
        return vec![];
    }

    let num_channels = layout.nb_channels as usize;
    let mut channels = vec![AudioChannel::Unknown; num_channels];

    match layout.order {
        ffi::AVChannelOrder::AV_CHANNEL_ORDER_NATIVE => {
            let mask = unsafe { layout.u.mask };
            let mut idx = 0;

            for (ffi_ch, ch) in CHANNEL_MAPPING {
                if (mask & (1 << (ffi_ch as u32))) > 0 {
                    channels[idx] = ch;
                    idx += 1;
                }
            }

            channels
        }
        ffi::AVChannelOrder::AV_CHANNEL_ORDER_CUSTOM => {
            let map = unsafe { std::slice::from_raw_parts(layout.u.map, num_channels) };

            for (idx, entry) in map.iter().enumerate() {
                let entry_ch = entry.id;
                for (ffi_ch, ch) in CHANNEL_MAPPING {
                    if ffi_ch == entry_ch {
                        channels[idx] = ch;
                        break;
                    }
                }
            }

            channels
        }
        _ => channels,
    }
}

fn convert_sample_format(format: ffi::AVSampleFormat) -> SampleFormat {
    match format {
        ffi::AVSampleFormat::AV_SAMPLE_FMT_U8 => SampleFormat::U8,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_S16 => SampleFormat::I16,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT => SampleFormat::F32,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_DBL => SampleFormat::F64,

        ffi::AVSampleFormat::AV_SAMPLE_FMT_U8P => SampleFormat::U8,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_S16P => SampleFormat::I16,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_FLTP => SampleFormat::F32,
        ffi::AVSampleFormat::AV_SAMPLE_FMT_DBLP => SampleFormat::F64,

        _ => SampleFormat::Other,
    }
}
