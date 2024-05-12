use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use std::{io, mem, ptr};

use crossbeam_queue::SegQueue;

use crate::sync::SharedMemory;

const UNSIGNALED: u32 = 0;

const WAITING: u32 = 1;

const SIGNALED: u32 = 2;

fn futex_wait(
    futex: &AtomicU32,
    val: u32,
    deadline: Option<&libc::timespec>,
) -> io::Result<libc::c_long> {
    let res = unsafe {
        libc::syscall(
            libc::SYS_futex,
            futex,
            libc::FUTEX_WAIT_BITSET,
            val,
            deadline.map_or(ptr::null(), |v| v),
            ptr::null::<u32>(),
            libc::FUTEX_BITSET_MATCH_ANY,
        )
    };
    if res == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(res)
}

fn futex_wake(futex: &AtomicU32, val: u32) -> io::Result<libc::c_long> {
    let res = unsafe { libc::syscall(libc::SYS_futex, futex, libc::FUTEX_WAKE, val) };
    if res == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(res)
}

fn futex_waitv(waiters: &[FutexWaiter]) -> io::Result<libc::c_long> {
    let res = unsafe {
        libc::syscall(
            libc::SYS_futex_waitv,
            waiters.as_ptr(),
            waiters.len() as libc::c_uint,
            0 as libc::c_uint,
            ptr::null::<libc::timespec>(),
            libc::CLOCK_MONOTONIC,
        )
    };
    if res == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(res)
}

#[repr(C)]
struct FutexWaiter {
    val: u64,
    uaddr: u64,
    flags: u32,
    __reserved: u32,
}

#[derive(Clone)]
pub struct OsEvent {
    shm: Arc<SharedMemory>,
}

impl OsEvent {
    pub fn create(prefix: &str) -> io::Result<OsEvent> {
        let shm = Arc::new(SharedMemory::create(prefix, mem::size_of::<AtomicU32>())?);
        Ok(OsEvent { shm })
    }

    pub unsafe fn open(id: &str) -> io::Result<OsEvent> {
        let shm = Arc::new(SharedMemory::open(id)?);
        Ok(OsEvent { shm })
    }

    pub fn id(&self) -> &str {
        self.shm.id()
    }

    pub fn prefix(&self) -> &str {
        self.shm.prefix()
    }

    fn futex(&self) -> &AtomicU32 {
        unsafe { &*(self.shm.as_ptr() as *const AtomicU32) }
    }

    fn wait_maybe_timeout(&self, timeout: Option<Duration>) {
        let deadline = timeout.map(|timeout| {
            let mut deadline = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };

            unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut deadline) };

            deadline.tv_sec += timeout.as_secs() as i64;
            deadline.tv_nsec += timeout.subsec_nanos() as i64;
            if deadline.tv_nsec > 1_000_000_000 {
                deadline.tv_sec += 1;
                deadline.tv_nsec -= 1_000_000_000;
            }

            deadline
        });

        let futex = self.futex();
        let mut state = UNSIGNALED;

        loop {
            let new_state = match state {
                UNSIGNALED => WAITING,
                WAITING => WAITING,
                SIGNALED => UNSIGNALED,
                _ => unreachable!("inconsistent event state"),
            };

            state = match futex.compare_exchange(state, new_state, SeqCst, SeqCst) {
                Ok(_) => new_state,
                Err(v) => v,
            };

            if state == UNSIGNALED {
                break;
            }

            if state == WAITING {
                match futex_wait(self.futex(), state, deadline.as_ref()) {
                    Ok(_) => {}
                    Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {}
                    Err(e) if e.raw_os_error() == Some(libc::EINTR) => {}
                    Err(e) if e.raw_os_error() == Some(libc::ETIMEDOUT) => {}
                    Err(e) => panic!("futex_wait failed: {e}"),
                }
            }
        }
    }

    pub fn wait(&self) {
        self.wait_maybe_timeout(None)
    }

    pub fn wait_timeout(&self, timeout: Duration) {
        self.wait_maybe_timeout(Some(timeout))
    }

    pub fn poll_wait(&self, context: &mut Context) -> Poll<()> {
        let futex = self.futex();
        let mut state = UNSIGNALED;
        loop {
            let new_state = match state {
                UNSIGNALED => WAITING,
                WAITING => WAITING,
                SIGNALED => UNSIGNALED,
                _ => unreachable!("inconsistent event state"),
            };

            state = match futex.compare_exchange(state, new_state, SeqCst, SeqCst) {
                Ok(_) => new_state,
                Err(v) => v,
            };

            if state == UNSIGNALED {
                return Poll::Ready(());
            }

            if state == WAITING {
                Reactor::get().register(Registration {
                    waker: context.waker().clone(),
                    futex,
                });

                return Poll::Pending;
            }
        }
    }

    pub async fn wait_async(&self) {
        std::future::poll_fn(|context| self.poll_wait(context)).await;
    }

    pub fn signal(&self) {
        let futex = self.futex();
        if futex.compare_exchange(UNSIGNALED, SIGNALED, SeqCst, SeqCst) == Err(WAITING) {
            futex.store(SIGNALED, SeqCst);
            futex_wake(self.futex(), 1).unwrap();
        }
    }
}

impl Drop for OsEvent {
    fn drop(&mut self) {
        let reactor = Reactor::get();
        reactor.unregister(self.futex());
    }
}

static REACTOR: OnceLock<Arc<Reactor>> = OnceLock::new();

enum Action {
    Register(Registration),
    Unregister { futex: *const AtomicU32 },
}

unsafe impl Send for Action {}
unsafe impl Sync for Action {}

struct Registration {
    waker: Waker,
    futex: *const AtomicU32,
}

unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

struct Reactor {
    master_futex: AtomicU32,
    actions: SegQueue<Action>,
}

impl Reactor {
    #[cold]
    #[inline(never)]
    fn init() -> Arc<Reactor> {
        let reactor = Arc::new(Reactor {
            master_futex: AtomicU32::new(0),
            actions: SegQueue::new(),
        });

        let reactor_clone = reactor.clone();
        std::thread::Builder::new()
            .name("named-event-reactor".into())
            .spawn(move || {
                reactor_clone.run();
            })
            .unwrap();

        reactor
    }

    fn get() -> &'static Reactor {
        REACTOR.get_or_init(Reactor::init)
    }

    fn register(&self, registration: Registration) {
        self.actions.push(Action::Register(registration));
        futex_wake(&self.master_futex, 1).unwrap();
    }

    fn unregister(&self, futex: *const AtomicU32) {
        self.actions.push(Action::Unregister { futex });
        futex_wake(&self.master_futex, 1).unwrap();
    }

    fn run(&self) {
        let mut registrations = Vec::<Registration>::new();
        let mut waiters = Vec::new();
        loop {
            while let Some(action) = self.actions.pop() {
                match action {
                    Action::Register(reg) => registrations.push(reg),
                    Action::Unregister { futex } => registrations.retain(|v| v.futex != futex),
                }
            }

            waiters.clear();
            waiters.push(FutexWaiter {
                val: 0,
                uaddr: &self.master_futex as *const _ as usize as u64,
                flags: 2, // FUTEX_32
                __reserved: 0,
            });

            for reg in &registrations {
                waiters.push(FutexWaiter {
                    val: WAITING as u64,
                    uaddr: reg.futex as usize as u64,
                    flags: 2, // FUTEX_32
                    __reserved: 0,
                });
            }

            match futex_waitv(&waiters) {
                Ok(0) => {}
                Ok(v) => {
                    let idx = (v - 1) as usize;
                    let reg = registrations.swap_remove(idx);
                    reg.waker.wake();
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {
                    registrations.retain(|reg| {
                        if unsafe { (*reg.futex).load(SeqCst) } == WAITING {
                            true
                        } else {
                            reg.waker.wake_by_ref();
                            false
                        }
                    });
                }
                Err(e) if e.raw_os_error() == Some(libc::ETIMEDOUT) => continue,
                Err(e) if e.raw_os_error() == Some(libc::EINTR) => continue,
                Err(e) => panic!("futex_waitv failed: {e}"),
            }
        }
    }
}
