use std::alloc::Layout;
use std::io;
use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
#[cfg(not(loom))]
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
#[cfg(not(loom))]
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};

use crossbeam_utils::CachePadded;
#[cfg(loom)]
use loom::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};

use super::{Buffer, Consumer, Producer};
use crate::sync::{IpcSafe, SharedMemory};

pub type IpcProducer<T, U = ()> = Producer<T, U, IpcBuffer<T, U>>;

pub type IpcConsumer<T, U = ()> = Consumer<T, U, IpcBuffer<T, U>>;

pub struct IpcRing<T, U> {
    buffer: IpcBuffer<T, U>,
}

impl<T: IpcSafe, U: IpcSafe> IpcRing<T, U> {
    /// Creates an IPC ring buffer with a specified ID prefix.
    ///
    /// The rest of the ID will be randomly generated.
    pub fn create(prefix: &str, capacity: usize, userdata: U) -> io::Result<Self> {
        let buffer = IpcBuffer::create(prefix, capacity, userdata)?;
        Ok(Self { buffer })
    }

    /// Opens an ring buffer by ID.
    ///
    /// # Safety
    ///
    /// ID must be obtained by [`IpcRing::id`]
    pub unsafe fn open(id: &str) -> io::Result<Self> {
        let buffer = IpcBuffer::open(id)?;
        Ok(Self { buffer })
    }

    pub fn id(&self) -> &str {
        self.buffer.id()
    }

    pub fn prefix(&self) -> &str {
        self.buffer.prefix()
    }

    pub fn producer_created(&self) -> bool {
        self.buffer.header().producer_created.load(Acquire)
    }

    pub fn producer(self) -> IpcProducer<T, U> {
        let buffer = self.buffer;

        if buffer.header().producer_created.swap(true, AcqRel) {
            panic!("producer already created");
        }

        buffer.refcount().fetch_add(1, Relaxed);

        unsafe { Producer::new(ManuallyDrop::new(buffer)) }
    }

    pub fn consumer_created(&self) -> bool {
        self.buffer.header().consumer_created.load(Acquire)
    }

    pub fn consumer(self) -> IpcConsumer<T, U> {
        let buffer = self.buffer;

        if buffer.header().consumer_created.swap(true, AcqRel) {
            panic!("consumer already created");
        }

        buffer.refcount().fetch_add(1, Relaxed);

        unsafe { Consumer::new(ManuallyDrop::new(buffer)) }
    }
}

struct Header<U> {
    userdata: U,
    capacity: usize,
    read_state: CachePadded<AtomicUsize>,
    write_state: CachePadded<AtomicUsize>,
    refcount: AtomicU8,
    producer_created: AtomicBool,
    consumer_created: AtomicBool,
}

/// Ring buffer stored in shared memory
pub struct IpcBuffer<T, U> {
    shm: SharedMemory,
    marker: PhantomData<(T, U)>,
}

unsafe impl<T: Send + Sync, U: Send + Sync> Send for IpcBuffer<T, U> {}
unsafe impl<T: Send + Sync, U: Send + Sync> Sync for IpcBuffer<T, U> {}

impl<T: IpcSafe, U: IpcSafe> IpcBuffer<T, U> {
    fn create(prefix: &str, capacity: usize, userdata: U) -> io::Result<Self> {
        let layout = Self::layout(capacity);
        let shm = SharedMemory::create(prefix, layout.size())?;

        assert!(
            (shm.as_ptr() as usize) & (layout.align() - 1) == 0,
            "shm pointer must be aligned"
        );

        let header = Header {
            userdata,
            capacity,
            read_state: CachePadded::new(AtomicUsize::new(0)),
            write_state: CachePadded::new(AtomicUsize::new(0)),
            refcount: AtomicU8::new(0),
            producer_created: AtomicBool::new(false),
            consumer_created: AtomicBool::new(false),
        };

        // SAFETY: pointer is valid, enforced by [`SharedMemory::create`]
        unsafe { std::ptr::write(shm.as_ptr() as *mut Header<U>, header) };

        Ok(Self {
            shm,
            marker: PhantomData,
        })
    }

    unsafe fn open(id: &str) -> io::Result<Self> {
        let shm = SharedMemory::open(id)?;
        Ok(Self {
            shm,
            marker: PhantomData,
        })
    }

    fn id(&self) -> &str {
        self.shm.id()
    }

    fn prefix(&self) -> &str {
        self.shm.prefix()
    }

    fn header(&self) -> &Header<U> {
        // SAFETY: pointer is valid until self is dropped
        unsafe { &*(self.shm.as_ptr() as *const Header<U>) }
    }

    fn offset() -> usize {
        Layout::new::<Header<U>>()
            .extend(Layout::new::<T>())
            .unwrap()
            .1
    }

    fn layout(capacity: usize) -> Layout {
        Layout::new::<Header<U>>()
            .extend(Layout::array::<T>(capacity).unwrap())
            .unwrap()
            .0
    }
}

unsafe impl<T: IpcSafe, U: IpcSafe> Buffer<T, U> for IpcBuffer<T, U> {
    fn userdata(&self) -> &U {
        &self.header().userdata
    }

    fn data_ptr(&self) -> *mut MaybeUninit<T> {
        let offset = Self::offset();
        // SAFETY: after adding `offset`, pointer is still in bounds (since capacity is > 0)
        // and properly aligned (enforced by `Layout::extend`).
        unsafe { self.shm.as_ptr().add(offset) as *mut MaybeUninit<T> }
    }

    fn capacity(&self) -> usize {
        self.header().capacity
    }

    fn read_state(&self) -> &AtomicUsize {
        &self.header().read_state
    }

    fn write_state(&self) -> &AtomicUsize {
        &self.header().write_state
    }

    fn refcount(&self) -> &AtomicU8 {
        &self.header().refcount
    }
}
