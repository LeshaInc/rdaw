use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::sync::atomic::AtomicU8;
#[cfg(not(loom))]
use std::sync::atomic::AtomicUsize;

use crossbeam_utils::CachePadded;
#[cfg(loom)]
use loom::sync::atomic::AtomicUsize;

use super::{Buffer, INDEX_MASK};

struct Header<U> {
    userdata: U,
    capacity: usize,
    read_state: CachePadded<AtomicUsize>,
    write_state: CachePadded<AtomicUsize>,
    refcount: AtomicU8,
}

/// Ring buffer stored in a normal process-local allocation.
pub struct LocalBuffer<T, U = ()> {
    inner: NonNull<Header<U>>,
    marker: PhantomData<T>,
}

unsafe impl<T: Send + Sync, U: Send + Sync> Send for LocalBuffer<T, U> {}
unsafe impl<T: Send + Sync, U: Send + Sync> Sync for LocalBuffer<T, U> {}

impl<T, U> LocalBuffer<T, U> {
    pub fn new(capacity: usize, userdata: U) -> LocalBuffer<T, U> {
        assert!(capacity > 0 && capacity <= INDEX_MASK && capacity.is_power_of_two());

        let layout = Self::layout(capacity);

        // SAFETY: size cannot be zero because we're storing at least `Header`.
        let ptr = unsafe { std::alloc::alloc(layout) as *mut Header<U> };

        let Some(inner) = NonNull::new(ptr) else {
            std::alloc::handle_alloc_error(layout);
        };

        let header = Header {
            userdata,
            capacity,
            read_state: CachePadded::new(AtomicUsize::new(0)),
            write_state: CachePadded::new(AtomicUsize::new(0)),
            refcount: AtomicU8::new(2),
        };

        // SAFETY: pointer is valid, because we've just allocated it and checked the result.
        unsafe { std::ptr::write(inner.as_ptr(), header) };

        LocalBuffer {
            inner,
            marker: PhantomData,
        }
    }

    fn header(&self) -> &Header<U> {
        // SAFETY: pointer is valid until self is dropped
        unsafe { self.inner.as_ref() }
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

unsafe impl<T, U> Buffer<T, U> for LocalBuffer<T, U> {
    fn userdata(&self) -> &U {
        &self.header().userdata
    }

    fn data_ptr(&self) -> *mut MaybeUninit<T> {
        let offset = Self::offset();
        // SAFETY: after adding `offset`, pointer is still in bounds (since capacity is > 0)
        // and properly aligned (enforced by `Layout::extend`).
        unsafe { (self.inner.as_ptr() as *mut u8).add(offset) as *mut MaybeUninit<T> }
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

impl<T, U> Drop for LocalBuffer<T, U> {
    fn drop(&mut self) {
        let layout = Self::layout(self.capacity());
        // SAFETY: block of memory was allocated with `std::alloc::alloc` using the same `layout`.
        unsafe { std::alloc::dealloc(self.inner.as_ptr() as *mut u8, layout) };
    }
}
