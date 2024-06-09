mod cache;
mod ops;

use std::path::PathBuf;

use blake3::Hash;
use rdaw_api::blob::BlobId;
use rdaw_core::Uuid;

pub use self::cache::BlobCache;
use crate::Object;

#[derive(Debug, Clone)]
pub enum Blob {
    Internal {
        uuid: Uuid,
        hash: Hash,
    },
    External {
        uuid: Uuid,
        hash: Hash,
        path: PathBuf,
    },
}

impl Blob {
    pub fn new_internal(hash: Hash) -> Blob {
        Blob::Internal {
            uuid: Uuid::new_v4(),
            hash,
        }
    }

    pub fn new_external(hash: Hash, path: PathBuf) -> Blob {
        Blob::External {
            uuid: Uuid::new_v4(),
            hash,
            path,
        }
    }

    pub fn hash(&self) -> Hash {
        match *self {
            Blob::Internal { hash, .. } => hash,
            Blob::External { hash, .. } => hash,
        }
    }
}

impl Object for Blob {
    type Id = BlobId;

    fn uuid(&self) -> Uuid {
        match *self {
            Blob::Internal { uuid, .. } => uuid,
            Blob::External { uuid, .. } => uuid,
        }
    }
}
