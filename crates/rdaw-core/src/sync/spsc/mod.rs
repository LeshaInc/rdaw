mod ipc;
mod local;

use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crossbeam_utils::Backoff;
use futures::Stream;

pub use self::ipc::{IpcChannel, IpcReceiver, IpcSender, RawIpcReceiver, RawIpcSender};
pub use self::local::{RawLocalReceiver, RawLocalSender};

pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (raw_sender, raw_receiver) = self::local::channel(capacity);
    (
        Sender {
            raw: raw_sender,
            marker: PhantomData,
        },
        Receiver {
            raw: raw_receiver,
            marker: PhantomData,
        },
    )
}

pub trait RawSender<T> {
    fn refresh(&mut self);

    fn is_closed(&self) -> bool;

    fn send_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed>;

    fn send_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>>;

    fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>>;

    fn try_send_slice(&mut self, slice: &[T]) -> Result<(), TrySendError<()>>
    where
        T: Copy;
}

pub trait RawReceiver<T> {
    fn refresh(&mut self);

    fn is_closed(&self) -> bool;

    fn recv_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed>;

    fn recv_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>>;

    fn try_recv(&mut self) -> Result<T, TryRecvError>;

    fn try_recv_slice(&mut self, slice: &mut [T]) -> Result<(), TryRecvError>
    where
        T: Copy;
}

pub struct Sender<T, R: RawSender<T> = RawLocalSender<T>> {
    raw: R,
    marker: PhantomData<T>,
}

impl<T, R: RawSender<T>> Sender<T, R> {
    pub fn refresh(&mut self) {
        self.raw.refresh();
    }

    pub fn is_closed(&self) -> bool {
        self.raw.is_closed()
    }

    pub fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>> {
        self.raw.try_send(value)
    }

    fn send_deadline(
        &mut self,
        value: T,
        deadline: Option<Instant>,
    ) -> Result<(), TrySendError<T>> {
        let mut value = Some(value);
        let backoff = Backoff::new();

        loop {
            match self.raw.try_send(value.take().unwrap()) {
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
                        let res = self.raw.send_wait(1, deadline);
                        if let Err(Closed) = res {
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
            match self.raw.try_send(value.take().unwrap()) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(TrySendError::Closed(v)) => return Poll::Ready(Err(SendError::Closed(v))),
                Err(TrySendError::Full(v)) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.raw.send_wait_async(1, ctx) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(Closed)) => {
                                return Poll::Ready(Err(SendError::Closed(v)));
                            }
                            Poll::Pending => {
                                value = Some(v);
                                return Poll::Pending;
                            }
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
        self.raw.try_send_slice(slice)
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
            match self.raw.try_send_slice(slice) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Closed(())) => return Err(TrySendError::Closed(())),
                Err(TrySendError::Full(())) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        let res = self.raw.send_wait(slice.len(), deadline);
                        if let Err(Closed) = res {
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

    pub fn send_slice_async<'a>(
        &'a mut self,
        slice: &'a [T],
    ) -> impl Future<Output = Result<(), SendError<()>>> + 'a
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.raw.try_send_slice(slice) {
                Ok(()) => return Poll::Ready(Ok(())),
                Err(TrySendError::Closed(())) => return Poll::Ready(Err(SendError::Closed(()))),
                Err(TrySendError::Full(())) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.raw.send_wait_async(slice.len(), ctx) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(Closed)) => {
                                return Poll::Ready(Err(SendError::Closed(())));
                            }
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                }
            }
        })
    }
}

impl<T, R: RawSender<T> + Unpin> Unpin for Sender<T, R> {}

impl<T, R: RawSender<T>> fmt::Debug for Sender<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
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

pub struct Receiver<T, R: RawReceiver<T> = RawLocalReceiver<T>> {
    raw: R,
    marker: PhantomData<T>,
}

impl<T, R: RawReceiver<T>> Receiver<T, R> {
    pub fn refresh(&mut self) {
        self.raw.refresh();
    }

    pub fn is_closed(&self) -> bool {
        self.raw.is_closed()
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        self.raw.try_recv()
    }

    fn recv_deadline(&mut self, deadline: Option<Instant>) -> Result<T, TryRecvError> {
        let backoff = Backoff::new();

        loop {
            match self.raw.try_recv() {
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
                        let res = self.raw.recv_wait(1, deadline);
                        if let Err(Closed) = res {
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

    fn poll_recv(&mut self, ctx: &mut Context) -> Poll<Result<T, RecvError>> {
        let backoff = Backoff::new();

        loop {
            match self.raw.try_recv() {
                Ok(v) => return Poll::Ready(Ok(v)),
                Err(TryRecvError::Closed) => return Poll::Ready(Err(RecvError::Closed)),
                Err(TryRecvError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.raw.recv_wait_async(1, ctx) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(Closed)) => {
                                return Poll::Ready(Err(RecvError::Closed));
                            }
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                }
            }
        }
    }

    pub fn recv_async(&mut self) -> impl Future<Output = Result<T, RecvError>> + '_ {
        std::future::poll_fn(move |ctx| self.poll_recv(ctx))
    }

    pub fn try_recv_slice(&mut self, slice: &mut [T]) -> Result<(), TryRecvError>
    where
        T: Copy,
    {
        self.raw.try_recv_slice(slice)
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
            match self.raw.try_recv_slice(slice) {
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
                        let res = self.raw.recv_wait(slice.len(), deadline);
                        if let Err(Closed) = res {
                            return Err(TryRecvError::Closed);
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

    pub fn recv_slice_async<'a>(
        &'a mut self,
        slice: &'a mut [T],
    ) -> impl Future<Output = Result<(), RecvError>> + 'a
    where
        T: Copy,
    {
        let backoff = Backoff::new();

        std::future::poll_fn(move |ctx| loop {
            match self.raw.try_recv_slice(slice) {
                Ok(v) => return Poll::Ready(Ok(v)),
                Err(TryRecvError::Closed) => return Poll::Ready(Err(RecvError::Closed)),
                Err(TryRecvError::Empty) => {
                    #[cfg(not(loom))]
                    backoff.snooze();

                    if backoff.is_completed() || cfg!(loom) {
                        match self.raw.recv_wait_async(slice.len(), ctx) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(Closed)) => {
                                return Poll::Ready(Err(RecvError::Closed));
                            }
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                }
            }
        })
    }
}

impl<T, R: RawReceiver<T>> fmt::Debug for Receiver<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

impl<T, R: RawReceiver<T> + Unpin> Unpin for Receiver<T, R> {}

impl<T, R: RawReceiver<T> + Unpin> Stream for Receiver<T, R> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_recv(cx).map(|v| v.ok())
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[error("closed")]
pub struct Closed;

#[cfg(test)]
mod tests {
    #[cfg(not(loom))]
    use std::thread;

    #[cfg(loom)]
    use loom::thread;

    use super::*;

    #[test]
    #[cfg(not(loom))]
    fn seq() {
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
    fn slice_seq() {
        let (mut sender, mut receiver) = channel(4);

        assert!(sender.send_slice(&[1, 2, 3, 4]).is_ok());
        drop(sender);

        let mut buf = [0; 4];
        assert_eq!(receiver.recv_slice(&mut buf), Ok(()));
        assert_eq!(buf, [1, 2, 3, 4]);

        assert_eq!(receiver.recv(), Err(RecvError::Closed));
    }

    fn do_concurrent() {
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
    fn concurrent() {
        do_concurrent();
    }

    #[test]
    #[cfg(loom)]
    fn concurrent() {
        loom::model(do_concurrent);
    }
}
