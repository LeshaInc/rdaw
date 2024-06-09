use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct IdAllocator<I> {
    counter: AtomicU64,
    _marker: PhantomData<fn() -> I>,
}

impl<I> IdAllocator<I> {
    pub fn new() -> IdAllocator<I> {
        IdAllocator {
            counter: AtomicU64::new(0),
            _marker: PhantomData,
        }
    }

    pub fn next(&self) -> I
    where
        I: From<u64>,
    {
        I::from(self.counter.fetch_add(1, Ordering::Relaxed))
    }
}

impl<I> Default for IdAllocator<I> {
    fn default() -> Self {
        IdAllocator::new()
    }
}
