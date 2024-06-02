mod ipc;
mod local;

use std::fmt;
use std::marker::PhantomData;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::atomic::AtomicU8;
#[cfg(not(loom))]
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
#[cfg(not(loom))]
use std::sync::atomic::{fence, AtomicUsize};

#[cfg(loom)]
use loom::sync::atomic::Ordering::{Acquire, Relaxed, Release};
#[cfg(loom)]
use loom::sync::atomic::{fence, AtomicUsize};

pub use self::ipc::{IpcBuffer, IpcConsumer, IpcProducer, IpcRing};
pub use self::local::LocalBuffer;

/// Creates a lock-free SPSC ring buffer.
///
/// `capacity` must be a power of 2 between `1` and `usize::MAX / 2`.
pub fn buffer<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let buffer = ManuallyDrop::new(LocalBuffer::new(capacity, ()));

    // SAFETY: pointer is valid since we got it from a reference, `ManuallyDrop` will prevent double
    // drop. Inside, `ProcessLocalBuffer` is just a pointer, so it's fine to copy it.
    let buffer_copy = unsafe { std::ptr::read(&buffer) };

    // SAFETY: a newly created buffer will satisfy `read_state == write_state == 0`.
    let producer = unsafe { Producer::new(buffer_copy) };
    let consumer = unsafe { Consumer::new(buffer) };

    (producer, consumer)
}

/// Creates a lock-free SPSC ring buffer with the specified userdata.
///
/// `capacity` must be a power of 2 between `1` and `usize::MAX / 2`.
pub fn buffer_with_userdata<T, U>(
    capacity: usize,
    userdata: U,
) -> (Producer<T, U>, Consumer<T, U>) {
    let buffer = ManuallyDrop::new(LocalBuffer::new(capacity, userdata));

    // SAFETY: pointer is valid since we got it from a reference, `ManuallyDrop` will prevent double
    // drop. Inside, `ProcessLocalBuffer` is just a pointer, so it's fine to copy it.
    let buffer_copy = unsafe { std::ptr::read(&buffer) };

    // SAFETY: a newly created buffer will satisfy `read_state == write_state == 0`.
    let producer = unsafe { Producer::new(buffer_copy) };
    let consumer = unsafe { Consumer::new(buffer) };

    (producer, consumer)
}

/// Underlying storage of the ring buffer.
///
/// # Safety
///
/// `data_ptr()` must return a valid pointer to an array of at least `capacity` of `T`.
///
/// `capacity()` must return a power of two in the range of `[1, usize::MAX / 2]`.
pub unsafe trait Buffer<T, U> {
    fn userdata(&self) -> &U;

    /// Must be a valid pointer to an array of at least `capacity` of `T` (possibly uninitialized).
    fn data_ptr(&self) -> *mut MaybeUninit<T>;

    /// Capacity is in the range of `[1, usize::MAX / 2]`.
    fn capacity(&self) -> usize;

    /// Read state consists of the read index (lowest N-1 bits), and a flag (highest bit) signifying
    /// that the consumer is closed.
    fn read_state(&self) -> &AtomicUsize;

    /// Write state consists of the write index (lowest N-1 bits), and a flag (highest bit)
    /// signifying that the producer is closed.
    fn write_state(&self) -> &AtomicUsize;

    /// Reference count, should initially start at 2.
    fn refcount(&self) -> &AtomicU8;
}

const INDEX_MASK: usize = usize::MAX / 2;

const CLOSED_MASK: usize = 1 << (usize::BITS - 1);

/// Producing side of the SPSC ring buffer.
pub struct Producer<T, U = (), B: Buffer<T, U> = LocalBuffer<T, U>> {
    buffer: ManuallyDrop<B>,
    closed: bool,
    read_state: usize,
    write_idx: usize,
    marker: PhantomData<(T, U)>,
}

impl<T, U, B: Buffer<T, U>> Producer<T, U, B> {
    /// SAFETY: assuming `read_state == write_state == 0`.
    pub(super) unsafe fn new(buffer: ManuallyDrop<B>) -> Producer<T, U, B> {
        Producer {
            buffer,
            closed: false,
            read_state: 0,
            write_idx: 0,
            marker: PhantomData,
        }
    }

    /// Updates cached state, synchronizing with the consumer.
    ///
    /// Will affect the following methods:
    ///  - [`Self::len()`].
    ///  - [`Self::is_closed()`].
    ///  - [`Self::is_empty()`].
    ///  - [`Self::is_full()`].
    ///
    /// If the buffer isn't full but the consumer is droped, and the producer isn't aware of that,
    /// [`Self::push()`] will still allow pushing new values. without any errors.
    ///
    /// This is done for performance reasons.
    ///
    /// If you want to make sure this doesn't happen, call [`Self::refresh()`] before
    /// `Self::push()`.
    pub fn refresh(&mut self) {
        // Using `Acquire` here to establish a happens-after relationship with `Consumer::pop()` and
        // `Consumer::drop()`.
        self.read_state = self.buffer.read_state().load(Acquire);
    }

    fn read_idx(&self) -> usize {
        self.read_state & INDEX_MASK
    }

    /// Returns `true` if the buffer is closed (either manually, or by dropping
    /// [`Producer`] or [`Consumer`].
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_closed(&self) -> bool {
        self.closed || self.read_state & CLOSED_MASK == CLOSED_MASK
    }

    /// Returns the number of elements currently stored in the buffer.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn len(&self) -> usize {
        self.write_idx.wrapping_sub(self.read_idx()) & INDEX_MASK
    }

    /// Returns the capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    fn data_ptr(&self) -> *mut MaybeUninit<T> {
        self.buffer.data_ptr()
    }

    /// Returns a shared reference to userdata stored in the buffer.
    pub fn userdata(&self) -> &U {
        self.buffer.userdata()
    }

    /// Returns `true`, if the buffer is empty.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_empty(&self) -> bool {
        self.read_idx() == self.write_idx
    }

    /// Returns `true`, if the buffer is at full capacity.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Close the buffer, disallowing the [`Producer`] to push more values.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }

        self.closed = true;
        self.buffer
            .write_state()
            .store(self.write_idx | CLOSED_MASK, Relaxed);
    }

    fn ensure_free_slots(&mut self, num_slots: usize) -> Result<usize, PushError<()>> {
        if num_slots > self.capacity() - self.len() {
            self.refresh();
            if self.is_closed() {
                return Err(PushError::Closed(()));
            } else if num_slots > self.capacity() - self.len() {
                return Err(PushError::Full(()));
            }
        }

        // Now we are certain that there is space in the buffer for at least `num_slots` elements:
        //  - We are the sole producer, so no other thread can push more elements here (enforced by
        //    &mut).
        //  - The consumer can `pop()` some elements, but this would only free space at the start of
        //    the buffer, not affecting the end of the buffer, where we'll be writing.

        // `capacity` is a power of two, using bitwise and instead of modulo.
        let idx = self.write_idx & (self.capacity() - 1);

        Ok(idx)
    }

    /// SAFETY: `ensure_free_slots` must be called beforehand to ensure there is enough space for
    /// `num_slots` elements, and all of the newly added elements are properly initialized.
    unsafe fn commit(&mut self, num_slots: usize) {
        // Move the write index, marking this slot used, wrapping on overflow.
        self.write_idx = self.write_idx.wrapping_add(num_slots) & INDEX_MASK;

        // Update the write index in the shared buffer, notifying the consumer.
        // Using `Release` ordering to establish a happens-before relationship with
        // `Consumer::refresh()`.
        self.buffer.write_state().store(self.write_idx, Release);
    }

    /// Pushes an element into the ring buffer in a FIFO manner. The consumer will see elements in
    /// the same order.
    ///
    /// If the buffer is full, will update the cached state and check again.
    ///
    /// Then, if the buffer is closed, will return [`PushError::Closed`], and if it's full --
    /// [`PushError::Full`].
    ///
    /// Otherwise will store the element inside the ring buffer, returning `Ok(())`.
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        let idx = match self.ensure_free_slots(1) {
            Ok(v) => v,
            Err(PushError::Full(())) => return Err(PushError::Full(value)),
            Err(PushError::Closed(())) => return Err(PushError::Closed(value)),
        };

        // SAFETY: `data_ptr` is valid, `idx` is in bounds (ensured in `ensure_free_slots`)
        // The value is assumed to be uninitialized, so we make sure not to read it.
        unsafe { self.data_ptr().add(idx).write(MaybeUninit::new(value)) };

        // SAFETY: `ensure_free_slots` was called beforehand, new element is properly initialized.
        unsafe { self.commit(1) };

        Ok(())
    }

    /// Push a slice of elements, copying them into the ring buffer.
    ///
    /// Follows the same semantics as [`Self::push()`], returning `PushError::Full` if there is not
    /// enough space for the entire slice, and `PushError::Closed` if the buffer is closed.
    pub fn push_slice(&mut self, slice: &[T]) -> Result<(), PushError>
    where
        T: Copy,
    {
        let idx = self.ensure_free_slots(slice.len())?;

        // Compute how many elements will fit after `idx` without wrapping around
        let left_len = (self.capacity() - idx).min(slice.len());

        // Compute how many elements will wrap around the ring buffer
        let right_len = slice.len() - left_len;

        debug_assert!(left_len + right_len == slice.len());

        unsafe {
            // SAFETY: Pointers are valid, `idx` is in bounds up to `idx + left_len`
            std::ptr::copy_nonoverlapping(
                slice.as_ptr(),
                self.data_ptr().add(idx) as *mut T,
                left_len,
            );

            // SAFETY: Pointers are valid, `left_len` is in bounds up to `left_len + right_len`
            std::ptr::copy_nonoverlapping(
                slice.as_ptr().add(left_len),
                self.data_ptr() as *mut T,
                right_len,
            );
        }

        // SAFETY: `ensure_free_slots` was called beforehand, and we've copied
        // exactly `slice.len()` elements
        unsafe { self.commit(slice.len()) };

        Ok(())
    }
}

impl<T, U, B: Buffer<T, U>> Drop for Producer<T, U, B> {
    fn drop(&mut self) {
        self.close();

        if self.buffer.refcount().fetch_sub(1, Release) == 1 {
            fence(Acquire);
            self.refresh();

            let read_idx = self.read_idx();
            let write_idx = self.write_idx;

            // SAFETY: Refcount is 0, so the buffer can be dropped
            unsafe { drop_buffer(&mut self.buffer, read_idx, write_idx) };
        }
    }
}

impl<T, U, B: Buffer<T, U>> fmt::Debug for Producer<T, U, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Producer").finish_non_exhaustive()
    }
}

/// Error returned from [`Producer::push()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum PushError<T = ()> {
    /// The buffer is full, try again later.
    #[error("full")]
    Full(T),
    /// The buffer is closed.
    #[error("closed")]
    Closed(T),
}

/// Consuming side of the SPSC ring buffer.
pub struct Consumer<T, U = (), B: Buffer<T, U> = LocalBuffer<T, U>> {
    buffer: ManuallyDrop<B>,
    closed: bool,
    read_idx: usize,
    write_state: usize,
    marker: PhantomData<(T, U)>,
}

impl<T, U, B: Buffer<T, U>> Consumer<T, U, B> {
    /// SAFETY: assuming `read_state == write_state == 0`
    pub(super) unsafe fn new(buffer: ManuallyDrop<B>) -> Consumer<T, U, B> {
        Consumer {
            buffer,
            closed: false,
            read_idx: 0,
            write_state: 0,
            marker: PhantomData,
        }
    }

    /// Updates cached state, synchronizing with the producer.
    ///
    /// Will affect the following methods:
    ///  - [`Self::len()`].
    ///  - [`Self::is_closed()`].
    ///  - [`Self::is_empty()`].
    ///  - [`Self::is_full()`].
    ///
    /// No need to call before [`Self::pop()`]
    pub fn refresh(&mut self) {
        // Using `Acquire` here to establish a happens-after relationship with `Producer::pop()`
        // and `Producer::drop()`.
        self.write_state = self.buffer.write_state().load(Acquire);
    }

    fn write_idx(&self) -> usize {
        self.write_state & INDEX_MASK
    }

    /// Returns `true` if the buffer is closed (either manually, or by dropping
    /// [`Producer`] or [`Consumer`].
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_closed(&self) -> bool {
        self.closed || self.write_state & CLOSED_MASK == CLOSED_MASK
    }

    /// Returns the number of elements currently stored in the buffer.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn len(&self) -> usize {
        self.write_idx().wrapping_sub(self.read_idx) & INDEX_MASK
    }

    /// Returns the capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    fn data_ptr(&self) -> *mut MaybeUninit<T> {
        self.buffer.data_ptr()
    }

    /// Returns a shared reference to userdata stored in the buffer.
    pub fn userdata(&self) -> &U {
        self.buffer.userdata()
    }

    /// Returns `true`, if the buffer is empty.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_empty(&self) -> bool {
        self.write_idx() == self.read_idx
    }

    /// Returns `true`, if the buffer is at full capacity.
    ///
    /// Uses the cached state. To update, call [`Self::refresh()`].
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Close the buffer, disallowing the [`Producer`] to push more values.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }

        self.closed = true;
        self.buffer
            .read_state()
            .store(self.read_idx | CLOSED_MASK, Relaxed);
    }

    fn ensure_contains(&mut self, count: usize) -> Result<usize, PopError> {
        if self.len() < count {
            self.refresh();
            if self.len() < count {
                if self.is_closed() {
                    return Err(PopError::Closed);
                } else {
                    return Err(PopError::Empty);
                }
            }
        }

        // Now we are certain that there's at least `count` elements in the buffer:
        //  - We are the sole consumer, and no other thread can `pop()` here (enforced by &mut).
        //  - The producer can `push()`, affecting the end of the buffer, but not the start (we
        //    never overwrite elements).

        // `capacity` is a power of two, using bitwise and instead of modulo.
        let idx = self.read_idx & (self.capacity() - 1);

        Ok(idx)
    }

    /// SAFETY: `ensure_contains` must be called beforehand to ensure there are at least `count`
    /// elements.
    unsafe fn commit(&mut self, count: usize) {
        // Move the read index, wrapping on overflow
        self.read_idx = self.read_idx.wrapping_add(count) & INDEX_MASK;

        // Update the write index in the shared buffer, notifying the producer.
        // Using `Release` ordering to establish a happens-before relationship with
        // `Consumer::refresh`.
        self.buffer.read_state().store(self.read_idx, Release);
    }

    /// Pops an element from the buffer in a FIFO manner, i.e. the same order in which they have
    /// been pushed.
    ///
    /// If the buffer is empty, will update the cached state and check again.
    ///
    /// Then, if the buffer is still empty, will return either of [`PushError::Full`] or
    /// [`PushError::Closed`], depending on whether or not the producer still exists and more
    /// values can be pushed.
    ///
    /// If the buffer isn't empty, will pop the first value and return it.
    pub fn pop(&mut self) -> Result<T, PopError> {
        let idx = self.ensure_contains(1)?;

        // SAFETY: `data_ptr` is valid, `idx` is in its bounds, and the value is initialized
        // The value won't be read again because of `commit` on the next line
        let value = unsafe { self.data_ptr().add(idx).read().assume_init() };

        // SAFETY: `ensure_contains` called beforehand
        unsafe { self.commit(1) };

        Ok(value)
    }

    /// Pops multiple elements from the ring buffer, copying them into the specified slice.
    ///
    /// Follows the same semantics as [`Self::pop()`], returning `PopError::Empty` if there isn't at
    /// least `slice.len()` elements in the buffer , and `PopError::Closed` if the buffer is closed.
    pub fn pop_slice(&mut self, slice: &mut [T]) -> Result<(), PopError>
    where
        T: Copy,
    {
        let idx = self.ensure_contains(slice.len())?;

        // Compute how many elements will fit after `idx` without wrapping around
        let left_len = (self.capacity() - idx).min(slice.len());

        // Compute how many elements will wrap around the ring buffer
        let right_len = slice.len() - left_len;

        debug_assert!(left_len + right_len == slice.len());

        unsafe {
            // SAFETY: Pointers are valid, `idx` is in bounds up to `idx + left_len`
            // Assuming data is initialized (checked by `ensure_contains`).
            std::ptr::copy_nonoverlapping(
                self.data_ptr().add(idx) as *mut T,
                slice.as_mut_ptr(),
                left_len,
            );

            // SAFETY: Pointers are valid, `left_len` is in bounds up to `left_len + right_len`
            // Assuming data is initialized (checked by `ensure_contains`).
            std::ptr::copy_nonoverlapping(
                self.data_ptr() as *mut T,
                slice.as_mut_ptr().add(left_len),
                right_len,
            );
        }

        // SAFETY: `ensure_contains` was called beforehand, and we've copied
        // exactly `slice.len()` elements.
        unsafe { self.commit(slice.len()) };

        Ok(())
    }
}

impl<T, U, B: Buffer<T, U>> Drop for Consumer<T, U, B> {
    fn drop(&mut self) {
        self.close();

        if self.buffer.refcount().fetch_sub(1, Release) == 1 {
            fence(Acquire);
            self.refresh();

            let read_idx = self.read_idx;
            let write_idx = self.write_idx();

            // SAFETY: Refcount is 0, so the buffer can be dropped
            unsafe { drop_buffer(&mut self.buffer, read_idx, write_idx) };
        }
    }
}

impl<T, U, B: Buffer<T, U>> fmt::Debug for Consumer<T, U, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Consumer").finish_non_exhaustive()
    }
}

/// SAFETY: both producer and consumer should be closed, and only one of them should call
/// `drop_buffer`.
unsafe fn drop_buffer<T, U, B: Buffer<T, U>>(
    buffer: &mut ManuallyDrop<B>,
    mut read_idx: usize,
    write_idx: usize,
) {
    if std::mem::needs_drop::<T>() {
        while read_idx != write_idx {
            // `capacity` is a power of two, using bitwise and instead of modulo.
            let idx = read_idx & (buffer.capacity() - 1);

            // SAFETY: `data_ptr` is valid, `idx` is in its bounds, the value has been
            // initialized by the producer.
            unsafe { std::ptr::drop_in_place(buffer.data_ptr().add(idx)) };

            read_idx = read_idx.wrapping_add(1);
        }
    }

    // SAFETY: buffer can't be accessed afterwards.
    unsafe { ManuallyDrop::drop(buffer) };
}

/// Error returned from [`Consumer::pop()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum PopError {
    /// The buffer is empty, try again later.
    #[error("empty")]
    Empty,
    /// The buffer is closed.
    #[error("closed")]
    Closed,
}

#[cfg(test)]
mod tests {
    #[cfg(not(loom))]
    use std::thread;

    #[cfg(loom)]
    use loom::thread;

    use super::*;

    #[test]
    #[cfg(not(loom))]
    fn sequential_copy() {
        let (mut producer, mut consumer) = buffer(4);

        assert_eq!(producer.push(0), Ok(()));
        assert_eq!(consumer.pop(), Ok(0));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        assert_eq!(producer.push(1), Ok(()));
        assert_eq!(producer.push(2), Ok(()));
        assert_eq!(producer.push(3), Ok(()));
        assert_eq!(producer.push(4), Ok(()));
        assert_eq!(producer.push(5), Err(PushError::Full(5)));

        assert_eq!(consumer.pop(), Ok(1));
        assert_eq!(consumer.pop(), Ok(2));
        assert_eq!(consumer.pop(), Ok(3));
        assert_eq!(consumer.pop(), Ok(4));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        drop(producer);
        assert_eq!(consumer.pop(), Err(PopError::Closed));
    }

    #[test]
    #[cfg(not(loom))]
    fn sequential_zst() {
        let (mut producer, mut consumer) = buffer(4);

        assert_eq!(producer.push(()), Ok(()));
        assert_eq!(consumer.pop(), Ok(()));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        assert_eq!(producer.push(()), Ok(()));
        assert_eq!(producer.push(()), Ok(()));
        assert_eq!(producer.push(()), Ok(()));
        assert_eq!(producer.push(()), Ok(()));
        assert_eq!(producer.push(()), Err(PushError::Full(())));

        assert_eq!(consumer.pop(), Ok(()));
        assert_eq!(consumer.pop(), Ok(()));
        assert_eq!(consumer.pop(), Ok(()));
        assert_eq!(consumer.pop(), Ok(()));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        drop(producer);
        assert_eq!(consumer.pop(), Err(PopError::Closed));
    }

    #[test]
    #[cfg(not(loom))]
    fn sequential_drop() {
        let (mut producer, mut consumer) = buffer::<String>(4);

        assert_eq!(producer.push("0".into()), Ok(()));
        assert_eq!(consumer.pop(), Ok("0".into()));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        assert_eq!(producer.push("1".into()), Ok(()));
        assert_eq!(producer.push("2".into()), Ok(()));
        assert_eq!(producer.push("3".into()), Ok(()));
        assert_eq!(producer.push("4".into()), Ok(()));
        assert_eq!(producer.push("5".into()), Err(PushError::Full("5".into())));

        assert_eq!(consumer.pop(), Ok("1".into()));
        assert_eq!(consumer.pop(), Ok("2".into()));
        assert_eq!(consumer.pop(), Ok("3".into()));
        assert_eq!(consumer.pop(), Ok("4".into()));
        assert_eq!(consumer.pop(), Err(PopError::Empty));

        drop(producer);
        assert_eq!(consumer.pop(), Err(PopError::Closed));
    }

    #[test]
    #[cfg(not(loom))]
    fn push_pop_slice() {
        let (mut producer, mut consumer) = buffer(4);

        assert!(producer.push_slice(&[1, 2, 3, 4]).is_ok());

        let mut buf = [0; 4];

        assert!(consumer.pop_slice(&mut buf).is_ok());
        assert_eq!(buf, [1, 2, 3, 4]);

        assert_eq!(consumer.pop(), Err(PopError::Empty));
    }

    fn do_parallel_drop() {
        let (mut producer, mut consumer) = buffer::<String>(2);

        let expected_vec = vec!["1".to_string(), "2".to_string(), "3".to_string()];

        let vec = expected_vec.clone();
        let t1 = thread::spawn(move || {
            for value in vec {
                let mut value = Some(value);
                loop {
                    match producer.push(value.take().unwrap()) {
                        Ok(()) => break,
                        Err(PushError::Full(v)) => {
                            value = Some(v);
                            thread::yield_now();
                        }
                        Err(PushError::Closed(_)) => panic!("couldn't send all values"),
                    }
                }
            }
        });

        let t2 = thread::spawn(move || {
            let mut vec = Vec::new();
            loop {
                match consumer.pop() {
                    Ok(v) => vec.push(v),
                    Err(PopError::Empty) => {
                        thread::yield_now();
                    }
                    Err(PopError::Closed) => break,
                }
            }

            assert_eq!(vec, expected_vec);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    #[cfg(not(loom))]
    fn parallel_drop() {
        do_parallel_drop();
    }

    #[test]
    #[cfg(loom)]
    fn parallel_drop() {
        loom::model(do_parallel_drop);
    }
}
