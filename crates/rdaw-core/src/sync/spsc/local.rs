#[cfg(not(loom))]
use std::sync::atomic::Ordering::{Acquire, Release, SeqCst};
#[cfg(not(loom))]
use std::sync::atomic::{fence, AtomicUsize};
use std::task::{Context, Poll, Waker};
#[cfg(not(loom))]
use std::thread::{self, Thread};
use std::time::Instant;

use crossbeam_utils::atomic::AtomicCell;
use crossbeam_utils::CachePadded;
#[cfg(loom)]
use loom::sync::atomic::Ordering::{Acquire, Release, SeqCst};
#[cfg(loom)]
use loom::sync::atomic::{fence, AtomicUsize};
#[cfg(loom)]
use loom::thread::{self, Thread};

use super::{Closed, RawReceiver, RawSender, TryRecvError, TrySendError};
use crate::sync::ring::{buffer_with_userdata, Consumer, PopError, Producer, PushError};

pub fn channel<T>(capacity: usize) -> (RawLocalSender<T>, RawLocalReceiver<T>) {
    let (producer, consumer) = buffer_with_userdata(capacity, SharedState::default());
    (RawLocalSender { producer }, RawLocalReceiver { consumer })
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

pub struct RawLocalSender<T> {
    producer: Producer<T, SharedState>,
}

impl<T> RawLocalSender<T> {
    fn try_wake_receiver(&self) {
        if self.producer.len() >= self.producer.userdata().receiver_waiting_len.load(Acquire) {
            self.wake_receiver();
        }
    }

    fn wake_receiver(&self) {
        let thread = &self.producer.userdata().receiver_waker;
        if let Some(waker) = thread.take() {
            waker.wake();
        }
    }

    fn send_success(&mut self) {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(0, Release);
        self.try_wake_receiver();
    }
}

impl<T> RawSender<T> for RawLocalSender<T> {
    fn refresh(&mut self) {
        self.producer.refresh();
    }

    fn is_closed(&self) -> bool {
        self.producer.is_closed()
    }

    #[cold]
    fn send_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Sync(thread::current())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.is_closed() {
            return Err(Closed);
        }

        if self.producer.capacity() - self.producer.len() >= count {
            return Ok(());
        }

        self.try_wake_receiver();

        if let Some(deadline) = deadline {
            thread::park_timeout(Instant::now().saturating_duration_since(deadline))
        } else {
            thread::park();
        }

        Ok(())
    }

    #[cold]
    fn send_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Async(context.waker().clone())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.is_closed() {
            return Poll::Ready(Err(Closed));
        }

        if self.producer.capacity() - self.producer.len() >= count {
            return Poll::Ready(Ok(()));
        }

        self.try_wake_receiver();

        Poll::Pending
    }

    fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>> {
        self.producer.push(value).map_err(|e| match e {
            PushError::Full(v) => TrySendError::Full(v),
            PushError::Closed(v) => TrySendError::Closed(v),
        })?;

        self.send_success();

        Ok(())
    }

    fn try_send_slice(&mut self, slice: &[T]) -> Result<(), TrySendError<()>>
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
}

impl<T> Unpin for RawLocalSender<T> {}

impl<T> Drop for RawLocalSender<T> {
    fn drop(&mut self) {
        self.producer.close();
        fence(SeqCst);
        self.wake_receiver();
    }
}

pub struct RawLocalReceiver<T> {
    consumer: Consumer<T, SharedState>,
}

impl<T> RawLocalReceiver<T> {
    fn try_wake_sender(&self) {
        let free_space = self.consumer.capacity() - self.consumer.len();
        if free_space >= self.consumer.userdata().sender_waiting_len.load(Acquire) {
            self.wake_sender()
        }
    }

    #[cold]
    fn wake_sender(&self) {
        let thread = &self.consumer.userdata().sender_waker;
        if let Some(waker) = thread.take() {
            waker.wake();
        }
    }

    fn recv_success(&mut self) {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(0, Release);
        self.try_wake_sender();
    }
}

impl<T> RawReceiver<T> for RawLocalReceiver<T> {
    fn refresh(&mut self) {
        self.consumer.refresh();
    }

    fn is_closed(&self) -> bool {
        self.consumer.is_closed()
    }

    #[cold]
    fn recv_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed> {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);
        ud.receiver_waker
            .store(Some(AnyWaker::Sync(thread::current())));

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return Ok(());
        }

        if self.consumer.is_closed() {
            return Err(Closed);
        }

        self.try_wake_sender();

        if let Some(deadline) = deadline {
            thread::park_timeout(Instant::now().saturating_duration_since(deadline))
        } else {
            thread::park();
        }

        Ok(())
    }

    #[cold]
    fn recv_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>> {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);
        ud.receiver_waker
            .store(Some(AnyWaker::Async(context.waker().clone())));

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return Poll::Ready(Ok(()));
        }

        if self.consumer.is_closed() {
            return Poll::Ready(Err(Closed));
        }

        self.try_wake_sender();

        Poll::Pending
    }

    fn try_recv(&mut self) -> Result<T, TryRecvError> {
        let value = self.consumer.pop().map_err(|e| match e {
            PopError::Empty => TryRecvError::Empty,
            PopError::Closed => TryRecvError::Closed,
        })?;

        self.recv_success();

        Ok(value)
    }

    fn try_recv_slice(&mut self, slice: &mut [T]) -> Result<(), TryRecvError>
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
}

impl<T> Unpin for RawLocalReceiver<T> {}

impl<T> Drop for RawLocalReceiver<T> {
    fn drop(&mut self) {
        self.consumer.close();
        fence(SeqCst);
        self.wake_sender();
    }
}
