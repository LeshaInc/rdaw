use std::collections::VecDeque;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_lite::Stream;
use rdaw_core::collections::HashMap;
use rdaw_core::sync::spsc::{self, Receiver, Sender};

const CAPACITY: usize = 8;

#[derive(Debug)]
pub struct Subscribers<K, E> {
    map: HashMap<K, Entry<E>>,
}

#[derive(Debug)]
struct Entry<E> {
    senders: Vec<Sender<E>>,
    queue: VecDeque<E>,
}

impl<K, E> Subscribers<K, E> {
    pub fn new() -> Subscribers<K, E> {
        Subscribers {
            map: HashMap::default(),
        }
    }
}

impl<K: Copy + Eq + Hash, E: Clone> Subscribers<K, E> {
    pub fn subscribe(&mut self, key: K) -> Subscriber<E> {
        let (sender, receiver) = spsc::channel(CAPACITY);

        let entry = self.map.entry(key).or_insert_with(|| Entry {
            senders: Vec::new(),
            queue: VecDeque::new(),
        });

        entry.senders.push(sender);

        Subscriber { receiver }
    }

    pub fn notify(&mut self, key: K, event: E) {
        let Some(entry) = self.map.get_mut(&key) else {
            return;
        };

        if entry.senders.is_empty() {
            return;
        }

        entry.queue.push_back(event);
    }

    pub async fn update(&mut self) {
        for entry in self.map.values_mut() {
            if entry.senders.is_empty() {
                entry.queue.clear();
                continue;
            }

            let last_idx = entry.senders.len() - 1;

            for event in entry.queue.drain(..) {
                for sender in &mut entry.senders[..last_idx] {
                    let _ = sender.send_async(event.clone()).await;
                }

                let last_sender = &mut entry.senders[last_idx];
                let _ = last_sender.send_async(event.clone()).await;
            }
        }

        self.map.retain(|_, entry| {
            entry.senders.retain(|sender| !sender.is_closed());
            !entry.senders.is_empty()
        });
    }
}

impl<K, E> Default for Subscribers<K, E> {
    fn default() -> Subscribers<K, E> {
        Subscribers::new()
    }
}

#[derive(Debug)]
pub struct Subscriber<E> {
    receiver: Receiver<E>,
}

impl<E> Stream for Subscriber<E> {
    type Item = E;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}
