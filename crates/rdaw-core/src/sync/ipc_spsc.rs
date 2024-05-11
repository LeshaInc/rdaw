use std::cell::UnsafeCell;
use std::io;
use std::sync::atomic::Ordering::{Acquire, Release, SeqCst};
use std::sync::atomic::{fence, AtomicUsize};

use crossbeam_utils::{Backoff, CachePadded};

use super::ipc_event::Event;
use super::ipc_ring::{IpcBuffer, IpcRing};
use super::ring::{Consumer, PopError, Producer, PushError};
use super::IpcSafe;

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
        let sender_event = Event::create(self.ring.prefix())?;
        let id = sender_event.id();
        let producer = self.ring.producer();

        unsafe {
            let event_id = producer.userdata().sender_event_id.get();
            *event_id = (id.len(), str_to_array(id));
        }

        Ok(IpcSender {
            producer,
            sender_event,
            receiver_event: None,
        })
    }

    pub fn receiver(self) -> io::Result<IpcReceiver<T>> {
        let receiver_event = Event::create(self.ring.prefix())?;
        let id = receiver_event.id();
        let consumer = self.ring.consumer();

        unsafe {
            let event_id = consumer.userdata().receiver_event_id.get();
            *event_id = (id.len(), str_to_array(id));
        }

        Ok(IpcReceiver {
            consumer,
            sender_event: None,
            receiver_event,
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

pub struct IpcSender<T: IpcSafe> {
    producer: Producer<T, SharedState, IpcBuffer<T, SharedState>>,
    sender_event: Event,
    receiver_event: Option<Event>,
}

impl<T: IpcSafe> IpcSender<T> {
    fn try_wake_receiver(&mut self) {
        if self.producer.len() >= self.producer.userdata().receiver_waiting_len.load(Acquire) {
            self.wake_receiver();
        }
    }

    #[cold]
    #[inline(never)]
    fn wake_receiver(&mut self) {
        if self.receiver_event.is_none() {
            let ud = self.producer.userdata();
            let (id_len, id_arr) = unsafe { &*ud.receiver_event_id.get() };
            let id = std::str::from_utf8(&id_arr[..*id_len]).unwrap();
            let event = unsafe { Event::open(id) }.unwrap();
            self.receiver_event = Some(event);
        }

        self.receiver_event.as_ref().unwrap().signal();
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

        fence(SeqCst);

        self.producer.refresh();

        if self.producer.capacity() - self.producer.len() >= count {
            return Ok(());
        }

        if self.producer.is_closed() {
            return Err(Closed(()));
        }

        self.try_wake_receiver();
        self.sender_event.wait();

        Ok(())
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

impl<T: IpcSafe> Drop for IpcSender<T> {
    fn drop(&mut self) {
        self.producer.close();
        fence(SeqCst);
        self.wake_receiver();
    }
}

pub type TrySendError<T> = PushError<T>;

pub struct IpcReceiver<T: IpcSafe> {
    consumer: Consumer<T, SharedState, IpcBuffer<T, SharedState>>,
    sender_event: Option<Event>,
    receiver_event: Event,
}

impl<T: IpcSafe> IpcReceiver<T> {
    fn try_wake_sender(&mut self) {
        let free_space = self.consumer.capacity() - self.consumer.len();
        if free_space >= self.consumer.userdata().sender_waiting_len.load(Acquire) {
            self.wake_sender()
        }
    }

    #[cold]
    #[inline(never)]
    fn wake_sender(&mut self) {
        if self.sender_event.is_none() {
            let ud = self.consumer.userdata();
            let (id_len, id_arr) = unsafe { &*ud.sender_event_id.get() };
            let id = std::str::from_utf8(&id_arr[..*id_len]).unwrap();
            let event = unsafe { Event::open(id) }.unwrap();
            self.sender_event = Some(event);
        }

        self.sender_event.as_ref().unwrap().signal();
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

        fence(SeqCst);

        self.consumer.refresh();

        if self.consumer.len() >= count {
            return Ok(());
        }

        if self.consumer.is_closed() {
            return Err(Closed(()));
        }

        self.try_wake_sender();
        self.receiver_event.wait();

        Ok(())
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

impl<T: IpcSafe> Drop for IpcReceiver<T> {
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
