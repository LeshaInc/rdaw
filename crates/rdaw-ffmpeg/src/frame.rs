use std::marker::PhantomData;

use ffmpeg_sys_next as ffi;

use crate::{Error, Result};

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

    pub(crate) fn as_raw(&mut self) -> *mut ffi::AVFrame {
        self.raw
    }

    pub(crate) unsafe fn assume_filled(&mut self) -> FilledFrame<'_> {
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

pub struct FilledFrame<'a> {
    raw: *mut ffi::AVFrame,
    _packet: PhantomData<&'a mut Frame>,
}

impl Drop for FilledFrame<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::av_frame_unref(self.raw);
        }
    }
}
