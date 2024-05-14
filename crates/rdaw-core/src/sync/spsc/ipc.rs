use std::cell::UnsafeCell;
use std::io;
use std::marker::PhantomData;
use std::sync::atomic::Ordering::{Acquire, Release, SeqCst};
use std::sync::atomic::{fence, AtomicUsize};
use std::task::{Context, Poll};
use std::time::Instant;

use crossbeam_utils::CachePadded;

use super::{Closed, RawReceiver, RawSender, Receiver, Sender, TryRecvError, TrySendError};
use crate::sync::ring::{IpcConsumer, IpcProducer, IpcRing, PopError, PushError};
use crate::sync::{IpcSafe, NamedEvent};

pub type IpcSender<T> = Sender<T, RawIpcSender<T>>;

pub type IpcReceiver<T> = Receiver<T, RawIpcReceiver<T>>;

pub struct IpcChannel<T> {
    ring: IpcRing<T, SharedState>,
}

impl<T: IpcSafe> IpcChannel<T> {
    /// Creates an IPC SPSC channel with a specified ID prefix.
    ///
    /// The rest of the ID will be randomly generated.
    pub fn create(prefix: &str, capacity: usize) -> io::Result<Self> {
        let state = SharedState {
            sender_waiting_len: CachePadded::new(AtomicUsize::new(0)),
            sender_event_id: UnsafeCell::new((0, [0; 255])),
            receiver_waiting_len: CachePadded::new(AtomicUsize::new(0)),
            receiver_event_id: UnsafeCell::new((0, [0; 255])),
        };
        let ring = IpcRing::create(prefix, capacity, state)?;
        Ok(Self { ring })
    }

    /// Opens an IPC SPSC channel by ID.
    ///
    /// # Safety
    ///
    /// ID must be obtained by [`IpcChannel::id`]
    pub unsafe fn open(id: &str) -> io::Result<Self> {
        let ring = IpcRing::open(id)?;
        Ok(Self { ring })
    }

    pub fn id(&self) -> &str {
        self.ring.id()
    }

    pub fn sender(self) -> io::Result<IpcSender<T>> {
        let sender_event = NamedEvent::create(self.ring.prefix())?;
        let id = sender_event.id();
        let producer = self.ring.producer();

        unsafe {
            let event_id = producer.userdata().sender_event_id.get();
            *event_id = (id.len(), str_to_array(id));
        }

        Ok(Sender {
            raw: RawIpcSender {
                producer,
                sender_event,
                receiver_event: None,
            },
            marker: PhantomData,
        })
    }

    pub fn receiver(self) -> io::Result<IpcReceiver<T>> {
        let receiver_event = NamedEvent::create(self.ring.prefix())?;
        let id = receiver_event.id();
        let consumer = self.ring.consumer();

        unsafe {
            let event_id = consumer.userdata().receiver_event_id.get();
            *event_id = (id.len(), str_to_array(id));
        }

        Ok(Receiver {
            raw: RawIpcReceiver {
                consumer,
                sender_event: None,
                receiver_event,
            },
            marker: PhantomData,
        })
    }
}

struct SharedState {
    sender_waiting_len: CachePadded<AtomicUsize>,
    sender_event_id: UnsafeCell<(usize, [u8; 255])>,
    receiver_waiting_len: CachePadded<AtomicUsize>,
    receiver_event_id: UnsafeCell<(usize, [u8; 255])>,
}

/// SAFETY: Storing only POD types
unsafe impl IpcSafe for SharedState {}
unsafe impl Send for SharedState {}
unsafe impl Sync for SharedState {}

fn str_to_array(str: &str) -> [u8; 255] {
    let mut array = [0; 255];
    array[..str.len()].copy_from_slice(str.as_bytes());
    array
}

pub struct RawIpcSender<T: IpcSafe> {
    producer: IpcProducer<T, SharedState>,
    sender_event: NamedEvent,
    receiver_event: Option<NamedEvent>,
}

impl<T: IpcSafe> RawIpcSender<T> {
    fn try_wake_receiver(&mut self) {
        if self.producer.len() >= self.producer.userdata().receiver_waiting_len.load(Acquire) {
            self.wake_receiver();
        }
    }

    #[cold]
    fn wake_receiver(&mut self) {
        if self.receiver_event.is_none() {
            let ud = self.producer.userdata();
            let (id_len, id_arr) = unsafe { &*ud.receiver_event_id.get() };
            if *id_len == 0 {
                return;
            }
            let id = std::str::from_utf8(&id_arr[..*id_len]).unwrap();
            let event = unsafe { NamedEvent::open(id) }.unwrap();
            self.receiver_event = Some(event);
        }

        self.receiver_event.as_ref().unwrap().signal();
    }

    fn send_success(&mut self) {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(0, Release);
        self.try_wake_receiver();
    }
}

impl<T: IpcSafe> RawSender<T> for RawIpcSender<T> {
    #[cold]
    fn send_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);

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
            self.sender_event
                .wait_timeout(Instant::now().saturating_duration_since(deadline));
        } else {
            self.sender_event.wait();
        }

        Ok(())
    }

    #[cold]
    fn send_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>> {
        let ud = self.producer.userdata();
        ud.sender_waiting_len.store(count, Release);

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.is_closed() {
            return Poll::Ready(Err(Closed));
        }

        if self.producer.capacity() - self.producer.len() >= count {
            return Poll::Ready(Ok(()));
        }

        self.try_wake_receiver();
        self.sender_event.poll_wait(context).map(|_| Ok(()))
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

impl<T: IpcSafe> Drop for RawIpcSender<T> {
    fn drop(&mut self) {
        self.producer.close();
        fence(SeqCst);
        self.wake_receiver();
    }
}

pub struct RawIpcReceiver<T: IpcSafe> {
    consumer: IpcConsumer<T, SharedState>,
    sender_event: Option<NamedEvent>,
    receiver_event: NamedEvent,
}

impl<T: IpcSafe> RawIpcReceiver<T> {
    fn try_wake_sender(&mut self) {
        let free_space = self.consumer.capacity() - self.consumer.len();
        if free_space >= self.consumer.userdata().sender_waiting_len.load(Acquire) {
            self.wake_sender()
        }
    }

    #[cold]
    fn wake_sender(&mut self) {
        if self.sender_event.is_none() {
            let ud = self.consumer.userdata();
            let (id_len, id_arr) = unsafe { &*ud.sender_event_id.get() };
            if *id_len == 0 {
                return;
            }
            let id = std::str::from_utf8(&id_arr[..*id_len]).unwrap();
            let event = unsafe { NamedEvent::open(id) }.unwrap();
            self.sender_event = Some(event);
        }

        self.sender_event.as_ref().unwrap().signal();
    }

    fn recv_success(&mut self) {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(0, Release);
        self.try_wake_sender();
    }
}

impl<T: IpcSafe> RawReceiver<T> for RawIpcReceiver<T> {
    #[cold]
    fn recv_wait(&mut self, count: usize, deadline: Option<Instant>) -> Result<(), Closed> {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);

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
            self.receiver_event
                .wait_timeout(Instant::now().saturating_duration_since(deadline));
        } else {
            self.receiver_event.wait();
        }

        Ok(())
    }

    #[cold]
    fn recv_wait_async(&mut self, count: usize, context: &mut Context) -> Poll<Result<(), Closed>> {
        let ud = self.consumer.userdata();
        ud.receiver_waiting_len.store(count, Release);

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return Poll::Ready(Ok(()));
        }

        if self.consumer.is_closed() {
            return Poll::Ready(Err(Closed));
        }

        self.try_wake_sender();
        self.receiver_event.poll_wait(context).map(|_| Ok(()))
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

impl<T: IpcSafe> Drop for RawIpcReceiver<T> {
    fn drop(&mut self) {
        self.consumer.close();
        fence(SeqCst);
        self.wake_sender();
    }
}
