use std::ffi::{c_char, c_int, c_void};
use std::sync::Once;

use ffmpeg_sys_next as ffi;
use tracing::Level;

pub mod decoder;
pub mod error;
pub mod frame;
pub mod input;
pub mod packet;
pub mod reader;
pub mod resample;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct StreamIdx(pub c_int);

pub fn init() {
    static INIT: Once = Once::new();

    INIT.call_once(|| unsafe {
        ffi::av_log_set_level(ffi::AV_LOG_TRACE);
        ffi::av_log_set_flags(0);
        ffi::av_log_set_callback(Some(log_callback))
    });
}

unsafe extern "C" fn log_callback(
    ptr: *mut c_void,
    level: c_int,
    fmt: *const c_char,
    vl: *mut ffi::__va_list_tag,
) {
    let enabled = match level {
        ffi::AV_LOG_TRACE | ffi::AV_LOG_DEBUG => {
            tracing::event_enabled!(target: "ffmpeg", Level::TRACE)
        }
        ffi::AV_LOG_VERBOSE => tracing::event_enabled!(target: "ffmpeg", Level::DEBUG),
        ffi::AV_LOG_INFO => tracing::event_enabled!(target: "ffmpeg", Level::INFO),
        ffi::AV_LOG_WARNING => tracing::event_enabled!(target: "ffmpeg", Level::WARN),
        ffi::AV_LOG_ERROR | ffi::AV_LOG_FATAL | ffi::AV_LOG_PANIC => {
            tracing::event_enabled!(target: "ffmpeg", Level::ERROR)
        }
        _ => false,
    };

    if !enabled {
        return;
    }

    let mut print_prefix = 1;
    let mut buf = [0u8; 4096];

    let len = unsafe {
        ffi::av_log_format_line2(
            ptr,
            level,
            fmt,
            vl,
            buf.as_mut_ptr() as *mut _,
            4096,
            &mut print_prefix,
        )
    };

    let Ok(len) = usize::try_from(len) else {
        return;
    };

    let msg = String::from_utf8_lossy(&buf[..len.min(buf.len())]);
    let msg = msg.trim_end();

    match level {
        ffi::AV_LOG_TRACE | ffi::AV_LOG_DEBUG => {
            tracing::trace!(target: "ffmpeg", "{msg}")
        }
        ffi::AV_LOG_VERBOSE => tracing::debug!(target: "ffmpeg", "{msg}"),
        ffi::AV_LOG_INFO => tracing::info!(target: "ffmpeg", "{msg}"),
        ffi::AV_LOG_WARNING => tracing::warn!(target: "ffmpeg", "{msg}"),
        ffi::AV_LOG_ERROR | ffi::AV_LOG_FATAL | ffi::AV_LOG_PANIC => {
            tracing::error!(target: "ffmpeg", "{msg}")
        }
        _ => {}
    }
}
