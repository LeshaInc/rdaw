mod cache;
mod ops;

use std::path::PathBuf;

use blake3::Hash;
use rdaw_api::blob::BlobId;
use rdaw_api::Result;

pub use self::cache::BlobCache;
use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

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

    const TYPE: ObjectType = ObjectType::Blob;

    fn serialize(&self, _ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>> {
        todo!()
    }

    fn deserialize(_ctx: &mut DeserializationContext<'_>, _data: &[u8]) -> Result<Self> {
        todo!()
    }
}
