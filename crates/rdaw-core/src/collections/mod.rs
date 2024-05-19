pub type HashSet<K> = ahash::HashSet<K>;
pub type HashMap<K, V> = ahash::HashMap<K, V>;

pub type ImHashSet<K> = im::HashSet<K, ahash::RandomState>;
pub type ImHashMap<K, V> = im::HashMap<K, V, ahash::RandomState>;

pub type ImVec<T> = im::Vector<T>;
