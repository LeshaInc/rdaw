use std::ffi::{c_int, CStr};

use ffmpeg_sys_next as ffi;
use rdaw_api::{format_err, Error, ErrorKind};

pub mod input;
pub mod reader;

#[track_caller]
fn av_strerror(code: c_int) -> Error {
    let mut buf = [0u8; 256];
    unsafe { ffi::av_strerror(code, buf.as_mut_ptr() as *mut _, 256) };

    CStr::from_bytes_until_nul(&buf)
        .map(|v| {
            let msg = v.to_string_lossy();
            format_err!(ErrorKind::Other, "ffmpeg error: {msg}")
        })
        .unwrap_or_else(|_| format_err!(ErrorKind::Other, "ffmpeg error"))
}
