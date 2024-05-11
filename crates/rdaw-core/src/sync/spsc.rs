use std::future::Future;
#[cfg(not(loom))]
use std::sync::atomic::Ordering::{Acquire, Release, SeqCst};
#[cfg(not(loom))]
use std::sync::atomic::{fence, AtomicUsize};
use std::task::{Context, Poll, Waker};
#[cfg(not(loom))]
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use crossbeam_utils::atomic::AtomicCell;
use crossbeam_utils::{Backoff, CachePadded};
#[cfg(loom)]
use loom::sync::atomic::Ordering::{Acquire, Release, SeqCst};
#[cfg(loom)]
use loom::sync::atomic::{fence, AtomicUsize};
#[cfg(loom)]
use loom::thread::{self, Thread};

use super::ring::{buffer_with_userdata, Consumer, PopError, Producer, PushError};

pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (producer, consumer) = buffer_with_userdata(capacity, SharedState::default());
    (Sender { producer }, Receiver { consumer })
}

#[derive(Default)]
struct SharedState {
    sender_waiting_len: CachePadded<AtomicUsize>,
    sender_waker: AtomicCell<Option<AnyWaker>>,
    receiver_waiting_len: CachePadded<AtomicUsize>,
    receiver_waker: AtomicCell<Option<AnyWaker>>,
}

enum AnyWaker {
    Sync(Thread),
    Async(Waker),
}

impl AnyWaker {
    fn wake(self) {
        match self {
            AnyWaker::Sync(thread) => thread.unpark(),
            AnyWaker::Async(waker) => waker.wake(),
        }
    }
}

pub struct Sender<T> {
    producer: Producer<T, SharedState>,
}

impl<T> Sender<T> {
    fn try_wake_receiver(&self) {
        if self.producer.len() >= self.producer.userdata().receiver_waiting_len.load(Acquire) {
            self.wake_receiver();
        }
    }

    fn wake_receiver(&self) {
        wake(&self.producer.userdata().receiver_waker);
    }

    fn send_success(&mut self) {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(0, Release);
        self.try_wake_receiver();
    }

    #[cold]
    #[inline(never)]
    fn send_wait(&mut self, count: usize, deadline: Option<Instant>) -> bool {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Sync(thread::current())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.capacity() - self.producer.len() >= count {
            return true;
        }

        if self.producer.is_closed() {
            return false;
        }

        self.try_wake_receiver();

        if let Some(deadline) = deadline {
            thread::park_timeout(Instant::now().saturating_duration_since(deadline))
        } else {
            thread::park();
        }

        true
    }

    #[cold]
    #[inline(never)]
    fn send_wait_async(&mut self, count: usize, context: &Context) -> Poll<bool> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Async(context.waker().clone())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.capacity() - self.producer.len() >= count {
            return Poll::Ready(true);
        }

        if self.producer.is_closed() {
            return Poll::Ready(false);
        }

        self.try_wake_receiver();

        Poll::Pending
    }

    pub fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>> {
        self.producer.push(value).map_err(|e| match e {
            PushError::Full(v) => TrySendError::Full(v),
            PushError::Closed(v) => TrySendError::Closed(v),
        })?;

        self.send_success();

        Ok(())
    }

    fn send_deadline(
        &mut self,
        value: T,
        deadline: Option<Instant>,
    ) -> Result<(), TrySendError<T>> {
        let mut value = Some(value);
        let backoff = Backoff::new();

        loop {
            match self.try_send(value.take().unwrap()) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Closed(v)) => return Err(TrySendError::Closed(v)),
                Err(TrySendError::Full(v)) => {
                    if let Some(deadline) = deadline {
                        if Instant::now() > deadline {
                            return Err(TrySendError::Full(v));
                        }
                    }

                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        if !self.send_wait(1, deadline) {
                            return Err(TrySendError::Closed(v));
                        }
                    }

                    value = Some(v);
                }
            }
        }
    }

    pub fn send(&mut self, value: T) -> Result<(), SendError<T>> {
        self.send_deadline(value, None).map_err(|e| match e {
            TrySendError::Full(_) => unreachable!(),
            TrySendError::Closed(v) => SendError::Closed(v),
        })
    }

    pub fn send_timeout(&mut self, value: T, timeout: Duration) -> Result<(), TrySendError<T>> {
        self.send_deadline(value, Some(Instant::now() + timeout))
    }

    pub fn send_async(&mut self, value: T) -> impl Future<Output = Result<(), SendError<T>>> + '_ {
        let mut value = Some(value);
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.try_send(value.take().unwrap()) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(TrySendError::Closed(v)) => return Poll::Ready(Err(SendError::Closed(v))),
                Err(TrySendError::Full(v)) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.send_wait_async(1, ctx) {
                            Poll::Ready(true) => {}
                            Poll::Ready(false) => {
                                return Poll::Ready(Err(SendError::Closed(v)));
                            }
                            Poll::Pending => return Poll::Pending,
                        }
                    }

                    value = Some(v);
                }
            }
        })
    }

    pub fn try_send_slice(&mut self, slice: &[T]) -> Result<(), TrySendError<()>>
    where
        T: Copy,
    {
        self.producer.push_slice(slice).map_err(|e| match e {
            PushError::Full(v) => TrySendError::Full(v),
            PushError::Closed(v) => TrySendError::Closed(v),
        })?;

        self.send_success();

        Ok(())
    }

    fn send_slice_deadline(
        &mut self,
        slice: &[T],
        deadline: Option<Instant>,
    ) -> Result<(), TrySendError<()>>
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        loop {
            match self.try_send_slice(slice) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Closed(())) => return Err(TrySendError::Closed(())),
                Err(TrySendError::Full(())) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        if !self.send_wait(slice.len(), deadline) {
                            return Err(TrySendError::Closed(()));
                        }
                    }
                }
            }
        }
    }

    pub fn send_slice(&mut self, slice: &[T]) -> Result<(), SendError<()>>
    where
        T: Copy,
    {
        self.send_slice_deadline(slice, None).map_err(|e| match e {
            TrySendError::Full(_) => unreachable!(),
            TrySendError::Closed(v) => SendError::Closed(v),
        })
    }

    pub fn send_slice_timeout(
        &mut self,
        slice: &[T],
        timeout: Duration,
    ) -> Result<(), TrySendError<()>>
    where
        T: Copy,
    {
        self.send_slice_deadline(slice, Some(Instant::now() + timeout))
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.producer.close();
        fence(SeqCst);
        self.wake_receiver();
    }
}

/// Error returned from [`Sender::try_send()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum TrySendError<T> {
    /// The channel is full, try again later.
    #[error("full")]
    Full(T),
    /// The channel is closed.
    #[error("closed")]
    Closed(T),
}

/// Error returned from [`Sender::send()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum SendError<T> {
    /// The channel is closed.
    #[error("closed")]
    Closed(T),
}

pub struct Receiver<T> {
    consumer: Consumer<T, SharedState>,
}

impl<T> Receiver<T> {
    fn try_wake_sender(&self) {
        let free_space = self.consumer.capacity() - self.consumer.len();
        if free_space >= self.consumer.userdata().sender_waiting_len.load(Acquire) {
            self.wake_sender()
        }
    }

    fn wake_sender(&self) {
        wake(&self.consumer.userdata().sender_waker);
    }

    fn recv_success(&mut self) {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(0, Release);
        self.try_wake_sender();
    }

    #[cold]
    #[inline(never)]
    fn recv_wait(&mut self, count: usize, deadline: Option<Instant>) -> bool {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);
        ud.receiver_waker
            .store(Some(AnyWaker::Sync(thread::current())));

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return true;
        }

        if self.consumer.is_closed() {
            return false;
        }

        self.try_wake_sender();

        if let Some(deadline) = deadline {
            thread::park_timeout(Instant::now().saturating_duration_since(deadline))
        } else {
            thread::park();
        }

        true
    }

    #[cold]
    #[inline(never)]
    fn recv_wait_async(&mut self, count: usize, context: &Context) -> Poll<bool> {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);
        ud.receiver_waker
            .store(Some(AnyWaker::Async(context.waker().clone())));

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return Poll::Ready(true);
        }

        if self.consumer.is_closed() {
            return Poll::Ready(false);
        }

        self.try_wake_sender();

        Poll::Pending
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        let value = self.consumer.pop().map_err(|e| match e {
            PopError::Empty => TryRecvError::Empty,
            PopError::Closed => TryRecvError::Closed,
        })?;

        self.recv_success();

        Ok(value)
    }

    fn recv_deadline(&mut self, deadline: Option<Instant>) -> Result<T, TryRecvError> {
        let backoff = Backoff::new();

        loop {
            match self.try_recv() {
                Ok(v) => return Ok(v),
                Err(TryRecvError::Closed) => return Err(TryRecvError::Closed),
                Err(TryRecvError::Empty) => {
                    if let Some(deadline) = deadline {
                        if Instant::now() > deadline {
                            return Err(TryRecvError::Empty);
                        }
                    }

                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        if !self.recv_wait(1, deadline) {
                            return Err(TryRecvError::Closed);
                        }
                    }
                }
            }
        }
    }

    pub fn recv(&mut self) -> Result<T, RecvError> {
        self.recv_deadline(None).map_err(|e| match e {
            TryRecvError::Empty => unreachable!(),
            TryRecvError::Closed => RecvError::Closed,
        })
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<T, TryRecvError> {
        self.recv_deadline(Some(Instant::now() + timeout))
    }

    pub fn recv_async(&mut self) -> impl Future<Output = Result<T, RecvError>> + '_ {
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.try_recv() {
                Ok(v) => return Poll::Ready(Ok(v)),
                Err(TryRecvError::Closed) => return Poll::Ready(Err(RecvError::Closed)),
                Err(TryRecvError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.recv_wait_async(1, ctx) {
                            Poll::Ready(true) => {}
                            Poll::Ready(false) => return Poll::Ready(Err(RecvError::Closed)),
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                }
            }
        })
    }

    pub fn try_recv_slice(&mut self, slice: &mut [T]) -> Result<(), TryRecvError>
    where
        T: Copy,
    {
        self.consumer.pop_slice(slice).map_err(|e| match e {
            PopError::Empty => TryRecvError::Empty,
            PopError::Closed => TryRecvError::Closed,
        })?;

        self.recv_success();

        Ok(())
    }

    fn recv_slice_deadline(
        &mut self,
        slice: &mut [T],
        deadline: Option<Instant>,
    ) -> Result<(), TryRecvError>
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        loop {
            match self.try_recv_slice(slice) {
                Ok(()) => return Ok(()),
                Err(TryRecvError::Closed) => return Err(TryRecvError::Closed),
                Err(TryRecvError::Empty) => {
                    if let Some(deadline) = deadline {
                        if Instant::now() > deadline {
                            return Err(TryRecvError::Empty);
                        }
                    }

                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        if !self.recv_wait(slice.len(), deadline) {
                            return Err(TryRecvError::Empty);
                        }
                    }
                }
            }
        }
    }

    pub fn recv_slice(&mut self, slice: &mut [T]) -> Result<(), RecvError>
    where
        T: Copy,
    {
        self.recv_slice_deadline(slice, None).map_err(|e| match e {
            TryRecvError::Empty => unreachable!(),
            TryRecvError::Closed => RecvError::Closed,
        })
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.consumer.close();
        fence(SeqCst);
        self.wake_sender();
    }
}

/// Error returned from [`Receiver::try_recv()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum TryRecvError {
    /// The channel is empty, try again later.
    #[error("empty")]
    Empty,
    /// The channel is closed.
    #[error("closed")]
    Closed,
}

/// Error returned from [`Receiver::recv()`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum RecvError {
    /// The channel is closed.
    #[error("closed")]
    Closed,
}

#[cold]
#[inline(never)]
fn wake(thread: &AtomicCell<Option<AnyWaker>>) {
    if let Some(waker) = thread.take() {
        waker.wake();
    }
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
    fn test_seq() {
        let (mut sender, mut receiver) = channel(4);

        assert_eq!(sender.send(1), Ok(()));
        assert_eq!(sender.send(2), Ok(()));
        assert_eq!(sender.send(3), Ok(()));
        assert_eq!(sender.send(4), Ok(()));
        drop(sender);

        assert_eq!(receiver.recv(), Ok(1));
        assert_eq!(receiver.recv(), Ok(2));
        assert_eq!(receiver.recv(), Ok(3));
        assert_eq!(receiver.recv(), Ok(4));

        assert_eq!(receiver.recv(), Err(RecvError::Closed));
    }

    #[test]
    #[cfg(not(loom))]
    fn test_slice_seq() {
        let (mut sender, mut receiver) = channel(4);

        assert!(sender.send_slice(&[1, 2, 3, 4]).is_ok());
        drop(sender);

        let mut buf = [0; 4];
        assert_eq!(receiver.recv_slice(&mut buf), Ok(()));
        assert_eq!(buf, [1, 2, 3, 4]);

        assert_eq!(receiver.recv(), Err(RecvError::Closed));
    }

    fn concurrent() {
        let (mut sender, mut receiver) = channel(1);

        let t1 = thread::spawn(move || {
            assert_eq!(sender.send(1), Ok(()));
            assert_eq!(sender.send(2), Ok(()));
        });

        let t2 = thread::spawn(move || {
            assert_eq!(receiver.recv(), Ok(1));
            assert_eq!(receiver.recv(), Ok(2));
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    #[cfg(not(loom))]
    fn test_concurrent() {
        concurrent();
    }

    #[test]
    #[cfg(loom)]
    fn test_concurrent() {
        loom::model(concurrent);
    }
}
