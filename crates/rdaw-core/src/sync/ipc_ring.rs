use std::alloc::Layout;
use std::io;
use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::atomic::Ordering::{AcqRel, Relaxed};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};

use crossbeam_utils::CachePadded;

use super::ring::{Buffer, Consumer, Producer};
use super::shared_mem::SharedMemory;
use super::IpcSafe;

pub struct IpcRing<T, U> {
    buffer: Option<IpcBuffer<T, U>>,
}

impl<T: IpcSafe, U: IpcSafe> IpcRing<T, U> {
    pub fn create(prefix: String, capacity: usize, userdata: U) -> io::Result<Self> {
        let buffer = IpcBuffer::create(prefix, capacity, userdata)?;
        Ok(Self {
            buffer: Some(buffer),
        })
    }

    pub unsafe fn open(id: &str) -> io::Result<Self> {
        let buffer = IpcBuffer::open(id)?;
        Ok(Self {
            buffer: Some(buffer),
        })
    }

    pub fn id(&self) -> &str {
        self.buffer.as_ref().unwrap().id()
    }

    pub fn producer(mut self) -> Producer<T, U, IpcBuffer<T, U>> {
        let buffer = self.buffer.take().unwrap();

        if buffer.header().producer_created.swap(true, AcqRel) {
            panic!("producer already created");
        }

        buffer.refcount().fetch_add(1, Relaxed);

        unsafe { Producer::new(ManuallyDrop::new(buffer)) }
    }

    pub fn consumer(mut self) -> Consumer<T, U, IpcBuffer<T, U>> {
        let buffer = self.buffer.take().unwrap();

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

impl<T: IpcSafe, U: IpcSafe> IpcBuffer<T, U> {
    fn create(prefix: String, capacity: usize, userdata: U) -> io::Result<Self> {
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
        unsafe { (self.shm.as_ptr() as *mut u8).add(offset) as *mut MaybeUninit<T> }
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
