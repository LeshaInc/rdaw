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

impl FilledFrame<'_> {
    pub(crate) fn as_raw(&self) -> *const ffi::AVFrame {
        self.raw as _
    }

    pub(crate) unsafe fn get_f32_samples(&self) -> &[f32] {
        let num_samples =
            ((*self.raw).nb_samples as usize) * ((*self.raw).ch_layout.nb_channels as usize);
        std::slice::from_raw_parts((*self.raw).data[0] as *const f32, num_samples)
    }
}

impl Drop for FilledFrame<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::av_frame_unref(self.raw);
        }
    }
}
