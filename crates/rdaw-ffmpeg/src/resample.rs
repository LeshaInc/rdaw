use ffmpeg_sys_next as ffi;

use crate::{Error, FilledFrame, Frame, Result};

pub struct ResamplerConfig {
    pub in_ch_layout: ffi::AVChannelLayout,
    pub in_sample_fmt: ffi::AVSampleFormat,
    pub in_sample_rate: i32,
    pub out_ch_layout: ffi::AVChannelLayout,
    pub out_sample_fmt: ffi::AVSampleFormat,
    pub out_sample_rate: i32,
}

pub struct Resampler {
    raw: *mut ffi::SwrContext,
}

impl Resampler {
    pub(crate) fn new(config: ResamplerConfig) -> Result<Resampler> {
        let mut raw = std::ptr::null_mut();
        let res = unsafe {
            ffi::swr_alloc_set_opts2(
                &mut raw,
                &config.out_ch_layout,
                config.out_sample_fmt,
                config.out_sample_rate,
                &config.in_ch_layout,
                config.in_sample_fmt,
                config.in_sample_rate,
                0,
                std::ptr::null_mut(),
            )
        };
        if res < 0 {
            return Err(Error::new(res, "swr_alloc_set_opts2"));
        }

        Ok(Resampler { raw })
    }

    pub fn convert<'out>(
        &mut self,
        in_frame: FilledFrame<'_>,
        out_frame: &'out mut Frame,
    ) -> Result<FilledFrame<'out>> {
        let res =
            unsafe { ffi::swr_convert_frame(self.raw, out_frame.as_raw(), in_frame.as_raw()) };
        if res < 0 {
            return Err(Error::new(res, "swr_convert_frame"));
        }
        Ok(unsafe { out_frame.assume_filled() })
    }

    pub fn flush<'out>(&mut self, out_frame: &'out mut Frame) -> Result<FilledFrame<'out>> {
        let res = unsafe { ffi::swr_convert_frame(self.raw, out_frame.as_raw(), std::ptr::null()) };
        if res < 0 {
            return Err(Error::new(res, "swr_convert_frame"));
        }
        Ok(unsafe { out_frame.assume_filled() })
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe {
            ffi::swr_free(&mut self.raw);
        }
    }
}
