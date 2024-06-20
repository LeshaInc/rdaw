use std::marker::PhantomData;

use ffmpeg_sys_next as ffi;

use super::error::{Error, Result};

#[derive(Debug)]
pub struct Frame {
    raw: *mut ffi::AVFrame,
}

impl Frame {
    pub fn new() -> Result<Frame> {
        let raw = unsafe { ffi::av_frame_alloc() };
        if raw.is_null() {
            return Err(Error::new_oom("avframe"));
        }

        Ok(Frame { raw })
    }

    pub fn as_raw(&mut self) -> *mut ffi::AVFrame {
        self.raw
    }

    pub unsafe fn assume_filled(&mut self) -> FilledFrame<'_> {
        FilledFrame {
            raw: self.raw,
            _packet: PhantomData,
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            ffi::av_frame_free(&mut self.raw);
        }
    }
}

#[derive(Debug)]
pub struct FilledFrame<'a> {
    raw: *mut ffi::AVFrame,
    _packet: PhantomData<&'a mut Frame>,
}

impl FilledFrame<'_> {
    pub unsafe fn get_data(&self) -> &[u8] {
        let bytes_per_sample =
            ffi::av_get_bytes_per_sample(std::mem::transmute((*self.raw).format)) as usize;
        let len = ((*self.raw).nb_samples as usize)
            * ((*self.raw).ch_layout.nb_channels as usize)
            * bytes_per_sample;
        std::slice::from_raw_parts((*self.raw).data[0] as *const u8, len)
    }
}

impl Drop for FilledFrame<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::av_frame_unref(self.raw);
        }
    }
}
