use blake3::Hash;
use rdaw_api::Result;
use serde::{Deserialize, Serialize};

use super::{Asset, EmbeddedAsset, ExternalAsset};
use crate::define_version_enum;
use crate::document::encoding;
use crate::object::{DeserializationContext, SerializationContext};

pub fn serialize(_ctx: &mut SerializationContext<'_>, asset: &Asset) -> Result<Vec<u8>> {
    let raw = match asset {
        Asset::External(asset) => AssetLatest::External {
            path: &asset.path,
            hash: asset.hash,
        },
        Asset::Embedded(asset) => AssetLatest::Embedded { hash: asset.hash },
    };

    encoding::serialize(Version::LATEST.as_u32(), &raw)
}

pub fn deserialize(_ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Asset> {
    let (version, data) = encoding::extract_version(data)?;
    let raw = match Version::from_u32(version)? {
        Version::V1 => encoding::deserialize::<AssetLatest>(data)?,
    };

    let asset = match raw {
        AssetV1::External { path, hash } => Asset::External(ExternalAsset {
            path: path.into(),
            hash,
        }),
        AssetV1::Embedded { hash } => Asset::Embedded(EmbeddedAsset { hash }),
    };

    Ok(asset)
}

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}

type AssetLatest<'a> = AssetV1<'a>;

#[derive(Debug, Serialize, Deserialize)]
enum AssetV1<'a> {
    External { path: &'a str, hash: Hash },
    Embedded { hash: Hash },
}
