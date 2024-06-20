use std::ptr::{self, null_mut};

use ffmpeg_sys_next as ffi;

use super::error::{Error, Result};
use super::frame::{FilledFrame, Frame};
use super::packet::FilledPacket;

#[derive(Debug)]
pub struct Decoder {
    raw: *mut ffi::AVCodecContext,
}

impl Decoder {
    pub fn new(
        codec: *const ffi::AVCodec,
        codecpar: *const ffi::AVCodecParameters,
    ) -> Result<Decoder> {
        let raw = unsafe { ffi::avcodec_alloc_context3(codec) };
        if raw.is_null() {
            return Err(Error::new_oom("avcodec"));
        }

        let res = unsafe { ffi::avcodec_parameters_to_context(raw, codecpar) };
        if res < 0 {
            return Err(Error::new(res, "avcodec_open2"));
        }

        let res = unsafe { ffi::avcodec_open2(raw, codec, null_mut()) };
        if res < 0 {
            return Err(Error::new(res, "avcodec_open2"));
        }

        Ok(Decoder { raw })
    }

    pub fn send_packet(&mut self, mut packet: FilledPacket<'_>) -> Result<()> {
        let res = unsafe { ffi::avcodec_send_packet(self.raw, packet.as_raw()) };
        if res < 0 {
            return Err(Error::new(res, "avcodec_send_packet"));
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let res = unsafe { ffi::avcodec_send_packet(self.raw, ptr::null()) };
        if res < 0 {
            return Err(Error::new(res, "avcodec_send_packet"));
        }
        Ok(())
    }

    pub fn recv_frame<'a>(&mut self, frame: &'a mut Frame) -> Result<FilledFrame<'a>> {
        let res = unsafe { ffi::avcodec_receive_frame(self.raw, frame.as_raw()) };
        if res < 0 {
            return Err(Error::new(res, "avcodec_receive_frame"));
        }
        Ok(unsafe { frame.assume_filled() })
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            ffi::avcodec_free_context(&mut self.raw);
        }
    }
}
