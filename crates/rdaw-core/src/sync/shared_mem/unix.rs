use std::io;
use std::mem::ManuallyDrop;
use std::num::NonZeroUsize;
use std::os::fd::{AsRawFd, OwnedFd};
use std::ptr::NonNull;

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::mman::{mmap, munmap, shm_open, shm_unlink, MapFlags, ProtFlags};
use nix::sys::stat::{fstat, Mode};
use nix::unistd::ftruncate;
use rand::Rng;

pub struct OsShm {
    id: String,
    prefix: String,
    size: usize,
    ptr: NonNull<u8>,
    fd: ManuallyDrop<OwnedFd>,
    owner: bool,
}

unsafe impl Send for OsShm {}

unsafe impl Sync for OsShm {}

impl OsShm {
    pub fn create(prefix: &str, size: usize) -> io::Result<OsShm> {
        let mut rng = rand::thread_rng();

        let (fd, id) = loop {
            let id = format!("/{prefix}.{:08x}", rng.gen::<u32>());
            match shm_open(
                id.as_str(),
                // create only if doesn't exist, open for read and write
                OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_RDWR,
                // read and write for owner
                Mode::S_IRUSR | Mode::S_IWUSR,
            ) {
                Ok(fd) => break (fd, id),
                Err(Errno::EEXIST) => continue, // try again with a different name
                Err(e) => return Err(e.into()),
            };
        };

        let i64_size = i64::try_from(size).map_err(|_| io::Error::other("shm size > i64::MAX"))?;
        ftruncate(&fd, i64_size)?;

        let nz_size = NonZeroUsize::new(size).ok_or_else(|| io::Error::other("shm size == 0"))?;

        let ptr = unsafe {
            mmap(
                None, // desired address
                nz_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &fd,
                0, // offset into file
            )?
        };

        Ok(OsShm {
            id,
            prefix: prefix.into(),
            size,
            ptr: ptr.cast(),
            fd: ManuallyDrop::new(fd),
            owner: true,
        })
    }

    pub fn open(id: &str) -> io::Result<OsShm> {
        let fd = shm_open(id, OFlag::O_RDWR, Mode::S_IRUSR)?;

        let i64_size = fstat(fd.as_raw_fd())?.st_size;

        let size = usize::try_from(i64_size)
            .map_err(|_| io::Error::other("shm size doesn't fit into usize"))?;

        let nz_size = NonZeroUsize::new(size).ok_or_else(|| io::Error::other("shm size == 0"))?;

        let ptr = unsafe {
            mmap(
                None, // desired address
                nz_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &fd,
                0, // offset into file
            )?
        };

        let (prefix, _) = id.rsplit_once('.').unwrap();
        let prefix = prefix.strip_prefix('/').unwrap();

        Ok(OsShm {
            id: id.into(),
            prefix: prefix.into(),
            size,
            ptr: ptr.cast(),
            fd: ManuallyDrop::new(fd),
            owner: false,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for OsShm {
    fn drop(&mut self) {
        if let Err(e) = unsafe { munmap(self.ptr.cast(), self.size) } {
            tracing::error!("munmap failed: {e}");
        }

        unsafe { ManuallyDrop::drop(&mut self.fd) };

        if self.owner {
            if let Err(e) = shm_unlink(self.id.as_str()) {
                tracing::error!("shm_unlink failed: {e}")
            }
        }
    }
}
