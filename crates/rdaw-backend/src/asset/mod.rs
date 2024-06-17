mod encoding;
mod ops;

use blake3::Hash;
use rdaw_api::asset::AssetId;
use rdaw_api::Result;

use crate::object::{DeserializationContext, Object, ObjectId, ObjectType, SerializationContext};

impl ObjectId for AssetId {
    type Object = Asset;
}

#[derive(Debug, Clone)]
pub enum Asset {
    External(ExternalAsset),
    Embedded(EmbeddedAsset),
}

#[derive(Debug, Clone)]
pub struct ExternalAsset {
    pub path: String,
    pub hash: Hash,
}

#[derive(Debug, Clone)]
pub struct EmbeddedAsset {
    pub hash: Hash,
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
