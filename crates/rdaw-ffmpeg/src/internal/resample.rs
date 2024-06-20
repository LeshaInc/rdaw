use ffmpeg_sys_next as ffi;

use super::error::{Error, Result};

pub struct ResamplerConfig<'a> {
    pub in_ch_layout: &'a ffi::AVChannelLayout,
    pub in_sample_format: ffi::AVSampleFormat,
    pub in_sample_rate: i32,
    pub out_ch_layout: &'a ffi::AVChannelLayout,
    pub out_sample_format: ffi::AVSampleFormat,
    pub out_sample_rate: i32,
}

#[derive(Debug)]
pub struct Resampler {
    raw: *mut ffi::SwrContext,
    bytes_per_in_sample: usize,
    bytes_per_out_sample: usize,
    num_out_channels: usize,
    buf: Vec<u8>,
}

impl Resampler {
    pub fn new(config: ResamplerConfig) -> Result<Resampler> {
        let mut raw = std::ptr::null_mut();
        let res = unsafe {
            ffi::swr_alloc_set_opts2(
                &mut raw,
                config.out_ch_layout,
                config.out_sample_format,
                config.out_sample_rate,
                config.in_ch_layout,
                config.in_sample_format,
                config.in_sample_rate,
                0,
                std::ptr::null_mut(),
            )
        };
        if res < 0 {
            return Err(Error::new(res, "swr_alloc_set_opts2"));
        }

        let res = unsafe { ffi::swr_init(raw) };
        if res < 0 {
            return Err(Error::new(res, "swr_init"));
        }

        let bytes_per_in_sample =
            unsafe { ffi::av_get_bytes_per_sample(config.in_sample_format) as usize };
        let bytes_per_out_sample =
            unsafe { ffi::av_get_bytes_per_sample(config.out_sample_format) as usize };

        let delay = unsafe { ffi::swr_get_delay(raw, config.out_sample_rate.into()) };
        let buf = Vec::with_capacity((delay as usize + 3) * bytes_per_out_sample + 16);

        Ok(Resampler {
            raw,
            bytes_per_in_sample,
            bytes_per_out_sample,
            num_out_channels: config.out_ch_layout.nb_channels as usize,
            buf,
        })
    }

    pub fn convert<'a>(&'a mut self, input: &'a [u8]) -> Result<&'a [u8]> {
        let in_samples = i32::try_from(input.len() / self.bytes_per_in_sample).unwrap();
        let out_samples_bound = unsafe { ffi::swr_get_out_samples(self.raw, in_samples) };

        self.buf.clear();
        self.buf
            .reserve((out_samples_bound as usize) * self.bytes_per_out_sample + 16);

        let out_samples =
            (self.buf.capacity() / self.bytes_per_out_sample / self.num_out_channels) as i32;

        let mut out_buffer_ptr = self.buf.as_mut_ptr();
        let align_offset = out_buffer_ptr.align_offset(self.bytes_per_out_sample);
        out_buffer_ptr = unsafe { out_buffer_ptr.add(align_offset) };

        let mut out_buffers = [out_buffer_ptr];
        let mut in_buffers = [input.as_ptr()];

        let res = unsafe {
            ffi::swr_convert(
                self.raw,
                out_buffers.as_mut_ptr(),
                out_samples,
                in_buffers.as_mut_ptr(),
                in_samples,
            )
        };
        if res < 0 {
            return Err(Error::new(res, "swr_convert"));
        }

        if in_buffers[0] == out_buffer_ptr {
            return Ok(input);
        }

        let len = (res as usize) * self.num_out_channels * self.bytes_per_out_sample;
        unsafe { self.buf.set_len(len + align_offset) };
        Ok(&self.buf[align_offset..align_offset + len])
    }

    pub fn flush(&mut self) -> Result<&[u8]> {
        self.buf.clear();

        let mut out_buffer_ptr = self.buf.as_mut_ptr();
        let align_offset = out_buffer_ptr.align_offset(self.bytes_per_out_sample);
        out_buffer_ptr = unsafe { out_buffer_ptr.add(align_offset) };

        let mut out_buffers = [out_buffer_ptr];

        let out_samples =
            (self.buf.capacity() / self.bytes_per_out_sample / self.num_out_channels) as i32;

        let res = unsafe {
            ffi::swr_convert(
                self.raw,
                out_buffers.as_mut_ptr(),
                out_samples,
                std::ptr::null_mut(),
                0,
            )
        };
        if res < 0 {
            return Err(Error::new(res, "swr_convert"));
        }

        let len = (res as usize) * self.num_out_channels * self.bytes_per_out_sample;
        unsafe { self.buf.set_len(len + align_offset) };
        Ok(&self.buf[align_offset..align_offset + len])
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe {
            ffi::swr_free(&mut self.raw);
        }
    }
}
