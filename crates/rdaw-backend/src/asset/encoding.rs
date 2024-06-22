use blake3::Hash;
use rdaw_api::Result;
use rdaw_core::path::Utf8Path;
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
            size: asset.size,
        },
        Asset::Embedded(asset) => AssetLatest::Embedded {
            hash: asset.hash,
            size: asset.size,
        },
    };

    encoding::serialize(Version::LATEST.as_u32(), &raw)
}

pub fn deserialize(_ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Asset> {
    let (version, data) = encoding::extract_version(data)?;
    let raw = match Version::from_u32(version)? {
        Version::V1 => encoding::deserialize::<AssetLatest>(data)?,
    };

    let asset = match raw {
        AssetV1::External { path, hash, size } => Asset::External(ExternalAsset {
            path: path.into(),
            hash,
            size,
        }),
        AssetV1::Embedded { hash, size } => Asset::Embedded(EmbeddedAsset { hash, size }),
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
    External {
        #[serde(borrow)]
        path: &'a Utf8Path,
        hash: Hash,
        size: u64,
    },
    Embedded {
        hash: Hash,
        size: u64,
    },
}
