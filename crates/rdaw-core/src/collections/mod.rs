pub use std::collections::{hash_map, hash_set};

pub use dashmap;

pub type HashSet<K> = ahash::HashSet<K>;
pub type HashMap<K, V> = ahash::HashMap<K, V>;

pub type ImHashSet<K> = im::HashSet<K, ahash::RandomState>;
pub type ImHashMap<K, V> = im::HashMap<K, V, ahash::RandomState>;

pub type ImVec<T> = im::Vector<T>;

pub type DashMap<K, V> = dashmap::DashMap<K, V, ahash::RandomState>;
pub type DashSet<K> = dashmap::DashSet<K, ahash::RandomState>;
