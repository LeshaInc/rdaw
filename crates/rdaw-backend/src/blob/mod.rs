mod cache;
mod ops;

use std::path::PathBuf;

use blake3::Hash;
use rdaw_api::blob::BlobId;

pub use self::cache::BlobCache;
use crate::document;
use crate::object::{Object, ObjectId, SerializationContext};

impl ObjectId for BlobId {
    type Object = Blob;
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

impl Object for Blob {
    type Id = BlobId;

    fn serialize(&self, _ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        todo!()
    }

    fn deserialize(
        _ctx: &crate::object::DeserializationContext<'_>,
        _data: &[u8],
    ) -> Result<Self, document::Error>
    where
        Self: Sized,
    {
        todo!()
    }
}
