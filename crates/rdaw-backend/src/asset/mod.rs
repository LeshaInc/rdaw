mod encoding;
mod ops;
mod reader;
#[cfg(test)]
mod tests;

use blake3::Hash;
use rdaw_api::asset::AssetId;
use rdaw_api::Result;
use rdaw_core::path::{Utf8Path, Utf8PathBuf};

pub use self::reader::AssetReader;
use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for AssetId {
    type Object = Asset;
}

#[derive(Debug, Clone)]
pub enum Asset {
    External(ExternalAsset),
    Embedded(EmbeddedAsset),
}

impl Asset {
    pub fn path(&self) -> Option<&Utf8Path> {
        match self {
            Asset::External(v) => Some(&v.path),
            Asset::Embedded(_) => None,
        }
    }

    pub fn hash(&self) -> Hash {
        match self {
            Asset::External(v) => v.hash,
            Asset::Embedded(v) => v.hash,
        }
    }

    pub fn size(&self) -> u64 {
        match self {
            Asset::External(v) => v.size,
            Asset::Embedded(v) => v.size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternalAsset {
    pub path: Utf8PathBuf,
    pub hash: Hash,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct EmbeddedAsset {
    pub hash: Hash,
    pub size: u64,
}

impl Object for Asset {
    type Id = AssetId;

    const TYPE: ObjectType = ObjectType::Asset;

    fn serialize(&self, ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>> {
        self::encoding::serialize(ctx, self)
    }

    fn deserialize(ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Self> {
        self::encoding::deserialize(ctx, data)
    }
}
