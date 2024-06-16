use rdaw_api::Result;
use serde::{Deserialize, Serialize};

use super::TempoMap;
use crate::define_version_enum;
use crate::document::encoding;
use crate::object::{DeserializationContext, SerializationContext};

pub fn serialize(_ctx: &mut SerializationContext<'_>, tempo_map: &TempoMap) -> Result<Vec<u8>> {
    let raw = TempoMapLatest {
        beats_per_minute: tempo_map.beats_per_minute,
    };

    encoding::serialize(Version::LATEST.as_u32(), &raw)
}

pub fn deserialize(_ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<TempoMap> {
    let (version, data) = encoding::extract_version(data)?;
    let raw = match Version::from_u32(version)? {
        Version::V1 => encoding::deserialize::<TempoMapV1>(data)?,
    };

    Ok(TempoMap::new(raw.beats_per_minute))
}

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}

type TempoMapLatest = TempoMapV1;

#[derive(Debug, Serialize, Deserialize)]
struct TempoMapV1 {
    beats_per_minute: f32,
}
