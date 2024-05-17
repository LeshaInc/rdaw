use std::path::PathBuf;

use blake3::Hash;

slotmap::new_key_type! {
    pub struct BlobId;
}

#[derive(Debug, Clone)]
pub enum Blob {
    Internal { hash: Hash },
    External { hash: Hash, path: PathBuf },
}

impl Blob {
    pub fn hash(&self) -> Hash {
        match *self {
            Blob::Internal { hash, .. } => hash,
            Blob::External { hash, .. } => hash,
        }
    }
}
