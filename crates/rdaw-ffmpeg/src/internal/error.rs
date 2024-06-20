use std::ffi::{c_int, CStr};
use std::fmt;

use ffmpeg_sys_next as ffi;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ErrorKind {
    Eof,
    Again,
    OutOfMemory,
    Other,
}

pub struct Error {
    code: c_int,
    context: &'static str,
}

impl Error {
    pub fn new(code: c_int, context: &'static str) -> Error {
        Error { code, context }
    }

    pub fn new_oom(context: &'static str) -> Error {
        Error {
            code: ffi::AVERROR(ffi::ENOMEM),
            context,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        let c = self.code;
        match self.code {
            _ if c == ffi::AVERROR_EOF => ErrorKind::Eof,
            _ if c == ffi::AVERROR(ffi::EAGAIN) => ErrorKind::Again,
            _ if c == ffi::AVERROR(ffi::ENOMEM) => ErrorKind::OutOfMemory,
            _ => ErrorKind::Other,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 256];
        unsafe { ffi::av_strerror(self.code, buf.as_mut_ptr() as *mut _, 256) };
        match CStr::from_bytes_until_nul(&buf) {
            Ok(msg) => {
                write!(f, "{}: {}", self.context, msg.to_string_lossy())
            }
            Err(_) => {
                write!(f, "{}: error code {}", self.context, self.code)
            }
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for Error {}

impl From<Error> for rdaw_api::Error {
    #[track_caller]
    fn from(error: Error) -> Self {
        let kind = match error.kind() {
            ErrorKind::OutOfMemory => rdaw_api::ErrorKind::OutOfMemory,
            _ => rdaw_api::ErrorKind::Other,
        };

        rdaw_api::Error::new(kind, format!("ffmpeg error: {error}"))
    }
}
