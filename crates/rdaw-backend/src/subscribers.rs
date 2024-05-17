use std::collections::HashMap;
use std::hash::Hash;

use futures_lite::Stream;
use rdaw_core::sync::spsc::{self, Sender};

const CAPACITY: usize = 8;

#[derive(Debug)]
pub struct Subscribers<K, E> {
    map: HashMap<K, Vec<Sender<E>>>,
}

impl<K, E> Subscribers<K, E> {
    pub fn new() -> Subscribers<K, E> {
        Subscribers {
            map: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash, E: Clone> Subscribers<K, E> {
    pub fn subscribe(&mut self, key: K) -> impl Stream<Item = E> {
        let (sender, receiver) = spsc::channel(CAPACITY);
        self.map.entry(key).or_default().push(sender);
        receiver
    }

    pub async fn notify(&mut self, key: K, event: E) {
        let Some(senders) = self.map.get_mut(&key) else {
            return;
        };

        let mut i = 0;
        while i < senders.len() {
            if i == senders.len() - 1 {
                if senders[i].send_async(event).await.is_err() {
                    senders.remove(i);
                }
                break;
            } else if senders[i].send_async(event.clone()).await.is_err() {
                senders.remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn cleanup(&mut self) {
        self.map.retain(|_, vec| {
            vec.retain(|sender| !sender.is_closed());
            !vec.is_empty()
        });
    }
}

impl<K, E> Default for Subscribers<K, E> {
    fn default() -> Subscribers<K, E> {
        Subscribers::new()
    }
}
