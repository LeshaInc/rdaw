use std::marker::PhantomData;

use ffmpeg_sys_next as ffi;

use super::error::{Error, Result};
use super::StreamIdx;

#[derive(Debug)]
pub struct Packet {
    raw: *mut ffi::AVPacket,
}

impl Packet {
    pub fn new() -> Result<Packet> {
        let raw = unsafe { ffi::av_packet_alloc() };
        if raw.is_null() {
            return Err(Error::new_oom("avpacket"));
        }

        Ok(Packet { raw })
    }

    pub fn as_raw(&mut self) -> *mut ffi::AVPacket {
        self.raw
    }

    pub unsafe fn assume_filled(&mut self) -> FilledPacket<'_> {
        FilledPacket {
            raw: self.raw,
            _packet: PhantomData,
        }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe {
            ffi::av_packet_free(&mut self.raw);
        }
    }
}

#[derive(Debug)]
pub struct FilledPacket<'a> {
    raw: *mut ffi::AVPacket,
    _packet: PhantomData<&'a mut Packet>,
}

impl FilledPacket<'_> {
    pub fn as_raw(&mut self) -> *mut ffi::AVPacket {
        self.raw
    }

    pub fn stream_idx(&self) -> StreamIdx {
        StreamIdx(unsafe { (*self.raw).stream_index })
    }
}

impl Drop for FilledPacket<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::av_packet_unref(self.raw);
        }
    }
}
