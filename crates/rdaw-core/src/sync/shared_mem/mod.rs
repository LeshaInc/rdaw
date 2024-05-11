#[cfg(unix)]
mod unix;

use std::io;

#[cfg(unix)]
use self::unix::*;

pub struct SharedMemory(OsShm);

impl SharedMemory {
    pub fn create(prefix: String, size: usize) -> io::Result<SharedMemory> {
        OsShm::create(prefix, size).map(SharedMemory)
    }

    pub fn open(id: &str) -> io::Result<SharedMemory> {
        OsShm::open(id).map(SharedMemory)
    }

    pub fn id(&self) -> &str {
        self.0.id()
    }

    pub fn size(&self) -> usize {
        self.0.size()
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }
}
