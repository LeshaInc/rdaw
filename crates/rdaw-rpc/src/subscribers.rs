use std::collections::VecDeque;
use std::hash::Hash;
use std::sync::Arc;

use rdaw_core::collections::HashMap;

use crate::transport::ServerTransport;
use crate::{Protocol, ServerMessage, StreamId, StreamIdAllocator};

#[derive(Debug)]
pub struct Subscribers<K, E> {
    id_allocator: Arc<StreamIdAllocator>,
    entries: HashMap<K, Entry<E>>,
    closed_entries: Vec<Entry<E>>,
    streams: HashMap<StreamId, K>,
}

#[derive(Debug)]
struct Entry<E> {
    streams: Vec<StreamId>,
    closed_streams: Vec<StreamId>,
    queue: VecDeque<E>,
}

impl<K, E> Subscribers<K, E> {
    pub fn new(id_allocator: Arc<StreamIdAllocator>) -> Subscribers<K, E> {
        Subscribers {
            id_allocator,
            entries: HashMap::default(),
            closed_entries: Vec::new(),
            streams: HashMap::default(),
        }
    }
}

impl<K: Copy + Eq + Hash, E: Clone> Subscribers<K, E> {
    pub fn subscribe(&mut self, key: K) -> StreamId {
        let stream = self.id_allocator.next();

        let entry = self.entries.entry(key).or_insert_with(|| Entry {
            streams: Vec::with_capacity(1),
            closed_streams: Vec::with_capacity(1),
            queue: VecDeque::new(),
        });

        entry.streams.push(stream);
        self.streams.insert(stream, key);

        stream
    }

    pub fn notify(&mut self, key: K, event: E) {
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };

        if entry.streams.is_empty() {
            return;
        }

        entry.queue.push_back(event);
    }

    pub fn find_key(&mut self, stream: StreamId) -> Option<K> {
        self.streams.get(&stream).copied()
    }

    pub fn close_all(&mut self, key: K) {
        if let Some(v) = self.entries.remove(&key) {
            self.closed_entries.push(v);
        }
    }

    pub fn close_one(&mut self, key: K, stream: StreamId) {
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };

        let Some(idx) = entry.streams.iter().position(|v| *v == stream) else {
            return;
        };

        entry.streams.remove(idx);
        entry.closed_streams.push(stream);

        self.streams.remove(&stream);
    }

    pub async fn deliver<P, T, C>(&mut self, transport: &T, converter: C) -> Result<(), P::Error>
    where
        P: Protocol,
        T: ServerTransport<P>,
        C: Fn(E) -> P::Event,
    {
        let mut to_remove = Vec::new();
        let mut to_close = Vec::new();

        for (key, entry) in self.entries.iter_mut() {
            if entry.streams.is_empty() {
                entry.queue.clear();
                to_remove.push(*key);
                continue;
            }

            for event in entry.queue.drain(..) {
                for &id in &entry.streams {
                    let payload = converter(event.clone());
                    transport.send(ServerMessage::Event { id, payload }).await?;
                }
            }

            for id in entry.closed_streams.drain(..) {
                to_close.push(id);
            }
        }

        for entry in self.closed_entries.drain(..) {
            to_close.extend(entry.streams);
            to_close.extend(entry.closed_streams);
        }

        for key in to_remove {
            self.entries.remove(&key);
        }

        for id in to_close {
            transport.send(ServerMessage::CloseStream { id }).await?;
        }

        Ok(())
    }
}
