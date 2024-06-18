use std::ffi::{c_int, c_void};
use std::io::{Read, Seek, SeekFrom};

use ffmpeg_sys_next as ffi;
use rdaw_api::{bail, ErrorKind, Result};

pub struct AVIOReaderContext<T> {
    raw: *mut ffi::AVIOContext,
    _reader: Box<T>,
}

impl<T: Read + Seek> AVIOReaderContext<T> {
    pub fn new(reader: T) -> Result<AVIOReaderContext<T>> {
        let mut reader = Box::new(reader);

        let buffer_size = 4096;
        let buffer = unsafe { ffi::av_malloc(buffer_size) };
        if buffer.is_null() {
            bail!(
                ErrorKind::OutOfMemory,
                "failed to allocate buffer for avio context"
            );
        }

        unsafe extern "C" fn read<T: Read + Seek>(
            opaque: *mut c_void,
            buf: *mut u8,
            buf_size: c_int,
        ) -> c_int {
            let Ok(buf_size) = usize::try_from(buf_size) else {
                return 0;
            };

            buf.write_bytes(0, buf_size);
            let buf = std::slice::from_raw_parts_mut(buf, buf_size);

            let reader = &mut *(opaque as *mut T);
            match reader.read(buf) {
                Ok(0) => ffi::AVERROR_EOF,
                Ok(v) => v as c_int,
                Err(error) => {
                    tracing::error!(?error, "avio read error");
                    if let Some(code) = error.raw_os_error() {
                        -code
                    } else {
                        ffi::AVERROR_EXTERNAL
                    }
                }
            }
        }

        unsafe extern "C" fn seek<T: Read + Seek>(
            opaque: *mut c_void,
            offset: i64,
            whence: c_int,
        ) -> i64 {
            let reader = &mut *(opaque as *mut T);

            let res = if whence == ffi::AVSEEK_SIZE {
                let mut get_size = || -> std::io::Result<u64> {
                    let pos = reader.stream_position()?;
                    let size = reader.seek(SeekFrom::End(0))?;
                    reader.seek(SeekFrom::Start(pos))?;
                    Ok(size)
                };

                get_size()
            } else {
                let from = match whence {
                    ffi::SEEK_SET => SeekFrom::Start(offset as u64),
                    ffi::SEEK_CUR => SeekFrom::Current(offset),
                    ffi::SEEK_END => SeekFrom::End(offset),
                    _ => {
                        return ffi::AVERROR_EXTERNAL.into();
                    }
                };

                reader.seek(from)
            };

            match res {
                Ok(v) => {
                    let Ok(v) = i64::try_from(v) else {
                        tracing::error!("avio seek result exceeded i64::MAX");
                        return ffi::AVERROR_EXTERNAL.into();
                    };
                    v
                }
                Err(error) => {
                    tracing::error!(?error, "avio seek error");
                    i64::from(if let Some(code) = error.raw_os_error() {
                        -code
                    } else {
                        ffi::AVERROR_EXTERNAL
                    })
                }
            }
        }

        let raw = unsafe {
            ffi::avio_alloc_context(
                buffer as *mut u8,
                buffer_size as i32,
                0, // not writable
                &mut *reader as *mut T as *mut _,
                Some(read::<T>),
                None,
                Some(seek::<T>),
            )
        };
        if raw.is_null() {
            bail!(ErrorKind::OutOfMemory, "failed to allocate avio context");
        }

        Ok(AVIOReaderContext {
            _reader: reader,
            raw,
        })
    }

    pub fn as_raw(&self) -> *mut ffi::AVIOContext {
        self.raw
    }
}

impl<T> Drop for AVIOReaderContext<T> {
    fn drop(&mut self) {
        unsafe {
            if !(*self.raw).buffer.is_null() {
                ffi::av_free((*self.raw).buffer as *mut _);
            }
            (*self.raw).buffer = std::ptr::null_mut();
            ffi::avio_context_free((&mut self.raw) as *mut _);
        }
    }
}
