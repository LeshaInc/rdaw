use std::io::{Read, Seek};
use std::ptr;

use ffmpeg_sys_next as ffi;

use crate::{Decoder, Error, FilledPacket, Packet, ReaderContext, Result, StreamIdx};

pub struct InputContext<T> {
    _reader: ReaderContext<T>,
    raw: *mut ffi::AVFormatContext,
}

impl<T: Read + Seek> InputContext<T> {
    pub fn new(reader: T) -> Result<InputContext<T>> {
        let mut reader = ReaderContext::new(reader)?;

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
        let decoder = Decoder::new(codec)?;

        Ok(Some((stream_idx, decoder)))
    }

    pub fn read_packet<'a>(&mut self, packet: &'a mut Packet) -> Result<FilledPacket<'a>, Error> {
        let res = unsafe { ffi::av_read_frame(self.raw, packet.as_raw()) };
        if res < 0 {
            return Err(Error::new(res, "av_read_frame"));
        }
        Ok(unsafe { packet.assume_filled() })
    }
}

impl<T> Drop for InputContext<T> {
    fn drop(&mut self) {
        unsafe {
            ffi::avformat_close_input(&mut self.raw);
        }
    }
}
