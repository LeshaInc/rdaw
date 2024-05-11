use std::alloc::Layout;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Relaxed};
use std::time::Duration;
use std::{io, ptr};

use crate::sync::shared_mem::SharedMemory;

struct SharedState {
    mutex: libc::pthread_mutex_t,
    cond: libc::pthread_cond_t,
    notified: bool,
    refcount: AtomicUsize,
}

pub struct OsEvent {
    shm: SharedMemory,
}

impl OsEvent {
    pub fn create(prefix: String) -> io::Result<OsEvent> {
        let layout = Layout::new::<SharedState>();
        let shm = SharedMemory::create(prefix, layout.size())?;

        assert!(
            (shm.as_ptr() as usize) & (layout.align() - 1) == 0,
            "shm pointer must be aligned"
        );

        unsafe {
            let state = shm.as_ptr() as *mut SharedState;
            libc::pthread_mutex_init(&mut (*state).mutex, ptr::null());
            libc::pthread_cond_init(&mut (*state).cond, ptr::null());
            (*state).notified = false;
            (*state).refcount = AtomicUsize::new(1);
        }

        Ok(OsEvent { shm })
    }

    pub unsafe fn open(id: &str) -> io::Result<OsEvent> {
        let shm = SharedMemory::open(id)?;

        unsafe {
            let state = shm.as_ptr() as *mut SharedState;
            (*state).refcount.fetch_add(1, Relaxed);
        }

        Ok(OsEvent { shm })
    }

    pub fn id(&self) -> &str {
        self.shm.id()
    }

    pub fn wait(&self) {
        unsafe {
            let state = self.shm.as_ptr() as *mut SharedState;
            libc::pthread_mutex_lock(&mut (*state).mutex);
            while !(*state).notified {
                libc::pthread_cond_wait(&mut (*state).cond, &mut (*state).mutex);
            }
            (*state).notified = false;
            libc::pthread_mutex_unlock(&mut (*state).mutex);
        }
    }

    pub fn wait_timeout(&self, timeout: Duration) {
        unsafe {
            let mut deadline = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };

            libc::clock_gettime(libc::CLOCK_REALTIME, &mut deadline);

            deadline.tv_sec += timeout.as_secs() as i64;
            deadline.tv_nsec += timeout.subsec_nanos() as i64;
            if deadline.tv_nsec > 1_000_000_000 {
                deadline.tv_sec += 1;
                deadline.tv_nsec -= 1_000_000_000;
            }

            let state = self.shm.as_ptr() as *mut SharedState;
            libc::pthread_mutex_lock(&mut (*state).mutex);
            while !(*state).notified {
                libc::pthread_cond_timedwait(&mut (*state).cond, &mut (*state).mutex, &deadline);
            }
            (*state).notified = false;
            libc::pthread_mutex_unlock(&mut (*state).mutex);
        }
    }

    pub fn signal(&self) {
        unsafe {
            let state = self.shm.as_ptr() as *mut SharedState;
            libc::pthread_mutex_lock(&mut (*state).mutex);
            (*state).notified = true;
            libc::pthread_cond_signal(&mut (*state).cond);
            libc::pthread_mutex_unlock(&mut (*state).mutex);
        }
    }
}

impl Drop for OsEvent {
    fn drop(&mut self) {
        unsafe {
            let state = self.shm.as_ptr() as *mut SharedState;

            if (*state).refcount.fetch_sub(1, AcqRel) == 1 {
                libc::pthread_mutex_destroy(&mut (*state).mutex);
                libc::pthread_cond_destroy(&mut (*state).cond);
            }
        }
    }
}
