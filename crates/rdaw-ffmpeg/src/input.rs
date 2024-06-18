use std::io::{Read, Seek};
use std::ptr;

use ffmpeg_sys_next as ffi;

use crate::reader::AVIOReaderContext;

pub struct InputContext<T> {
    _reader: AVIOReaderContext<T>,
    raw: *mut ffi::AVFormatContext,
}

impl<T: Read + Seek> InputContext<T> {
    pub fn new(reader: T) -> InputContext<T> {
        let reader = AVIOReaderContext::new(reader);

        let mut raw = unsafe { ffi::avformat_alloc_context() };
        if raw.is_null() {
            panic!("failed to allocate avformat context");
        }

        unsafe {
            (*raw).pb = reader.as_raw();
        }

        let res = unsafe {
            ffi::avformat_open_input(&mut raw, ptr::null(), ptr::null(), ptr::null_mut())
        };
        if res < 0 {
            panic!("failed to open avformat input context");
        }

        let res = unsafe { ffi::avformat_find_stream_info(raw, ptr::null_mut()) };
        if res < 0 {
            panic!("failed to get avformat stream info");
        }

        InputContext {
            _reader: reader,
            raw,
        }
    }
}

impl<T> Drop for InputContext<T> {
    fn drop(&mut self) {
        unsafe {
            ffi::avformat_close_input(&mut self.raw);
        }
    }
}
