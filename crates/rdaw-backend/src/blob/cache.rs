use std::fmt;

use blake3::Hash;
use rdaw_core::collections::HashMap;

#[derive(Default)]
pub struct BlobCache {
    map: HashMap<Hash, Vec<u8>>,
}

impl BlobCache {
    pub fn insert(&mut self, hash: Hash, data: Vec<u8>) {
        self.map.insert(hash, data);
    }
}

impl fmt::Debug for BlobCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlobCache").finish_non_exhaustive()
    }
}
