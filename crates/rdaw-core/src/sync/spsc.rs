use std::future::Future;
#[cfg(not(loom))]
use std::sync::atomic::Ordering::{Acquire, Release, SeqCst};
#[cfg(not(loom))]
use std::sync::atomic::{fence, AtomicUsize};
use std::task::{Context, Poll, Waker};
#[cfg(not(loom))]
use std::thread::{self, Thread};

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
    fn send_wait(&mut self, count: usize) -> Result<(), Closed<()>> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Sync(thread::current())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.capacity() - self.producer.len() >= count {
            return Ok(());
        }

        if self.producer.is_closed() {
            return Err(Closed(()));
        }

        self.try_wake_receiver();

        thread::park();

        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn send_wait_async(&mut self, count: usize, context: &Context) -> Poll<Result<(), Closed<()>>> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);
        ud.sender_waker
            .store(Some(AnyWaker::Async(context.waker().clone())));

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.capacity() - self.producer.len() >= count {
            return Poll::Ready(Ok(()));
        }

        if self.producer.is_closed() {
            return Poll::Ready(Err(Closed(())));
        }

        self.try_wake_receiver();

        Poll::Pending
    }

    pub fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>> {
        self.producer.push(value)?;
        self.send_success();
        Ok(())
    }

    pub fn send(&mut self, value: T) -> Result<(), Closed<T>> {
        let mut value = Some(value);
        let backoff = Backoff::new();

        loop {
            match self.try_send(value.take().unwrap()) {
                Ok(()) => return Ok(()),
                Err(PushError::Closed(v)) => return Err(Closed(v)),
                Err(PushError::Full(v)) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        if let Err(Closed(())) = self.send_wait(1) {
                            return Err(Closed(v));
                        }
                    }

                    value = Some(v);
                }
            }
        }
    }

    pub fn send_async(&mut self, value: T) -> impl Future<Output = Result<(), Closed<T>>> + '_ {
        let mut value = Some(value);
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.try_send(value.take().unwrap()) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(PushError::Closed(v)) => return Poll::Ready(Err(Closed(v))),
                Err(PushError::Full(v)) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.send_wait_async(1, ctx) {
                            Poll::Ready(Ok(_)) => {}
                            Poll::Ready(Err(_)) => {
                                return Poll::Ready(Err(Closed(v)));
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
        self.producer.push_slice(slice)?;
        self.send_success();
        Ok(())
    }

    pub fn send_slice(&mut self, slice: &[T]) -> Result<(), Closed>
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        loop {
            match self.try_send_slice(slice) {
                Ok(()) => return Ok(()),
                Err(PushError::Closed(())) => return Err(Closed(())),
                Err(PushError::Full(())) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        self.send_wait(slice.len())?;
                    }
                }
            }
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.producer.close();
        fence(SeqCst);
        self.wake_receiver();
    }
}

pub type TrySendError<T> = PushError<T>;

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
    fn recv_wait(&mut self, count: usize) -> Result<(), Closed> {
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
            return Err(Closed(()));
        }

        self.try_wake_sender();

        thread::park();

        Ok(())
    }

    #[cold]
    #[inline(never)]
    fn recv_wait_async(&mut self, count: usize, context: &Context) -> Poll<Result<(), Closed>> {
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
            return Poll::Ready(Err(Closed(())));
        }

        self.try_wake_sender();

        Poll::Pending
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        let value = self.consumer.pop()?;
        self.recv_success();
        Ok(value)
    }

    pub fn recv(&mut self) -> Result<T, Closed> {
        let backoff = Backoff::new();

        loop {
            match self.try_recv() {
                Ok(v) => return Ok(v),
                Err(PopError::Closed) => return Err(Closed(())),
                Err(PopError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        self.recv_wait(1)?;
                    }
                }
            }
        }
    }

    pub fn recv_async(&mut self) -> impl Future<Output = Result<T, Closed>> + '_ {
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.try_recv() {
                Ok(v) => return Poll::Ready(Ok(v)),
                Err(PopError::Closed) => return Poll::Ready(Err(Closed(()))),
                Err(PopError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.recv_wait_async(1, ctx) {
                            Poll::Ready(Ok(_)) => {}
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
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
        self.consumer.pop_slice(slice)?;
        self.recv_success();
        Ok(())
    }

    pub fn recv_slice(&mut self, slice: &mut [T]) -> Result<(), Closed>
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        loop {
            match self.try_recv_slice(slice) {
                Ok(()) => return Ok(()),
                Err(PopError::Closed) => return Err(Closed(())),
                Err(PopError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();
                    if backoff.is_completed() || cfg!(loom) {
                        self.recv_wait(slice.len())?;
                    }
                }
            }
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.consumer.close();
        fence(SeqCst);
        self.wake_sender();
    }
}

pub type TryRecvError = PopError;

/// Error returned when trying to send to or receive from a closed channel.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub struct Closed<T = ()>(pub T);

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

        assert_eq!(receiver.recv(), Err(Closed(())));
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

        assert_eq!(receiver.recv(), Err(Closed(())));
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
