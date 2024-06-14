use serde::{Deserialize, Serialize};

use super::Arrangement;
use crate::define_version_enum;
use crate::document::{encoding, Result};
use crate::object::{DeserializationContext, SerializationContext, Uuid};

pub fn serialize(ctx: &mut SerializationContext<'_>, arrangement: &Arrangement) -> Result<Vec<u8>> {
    let tempo_map_uuid = ctx.add_dep(arrangement.tempo_map_id)?;
    let main_track_uuid = ctx.add_dep(arrangement.main_track_id)?;

    let raw = ArrangementLatest {
        tempo_map_uuid,
        main_track_uuid,
        name: &arrangement.name,
    };

    encoding::serialize(Version::LATEST.as_u32(), &raw)
}

pub fn deserialize(ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Arrangement> {
    let (version, data) = encoding::extract_version(data)?;
    let raw = match Version::from_u32(version)? {
        Version::V1 => encoding::deserialize::<ArrangementV1>(data)?,
    };

    let tempo_map_id = ctx.add_dep(raw.tempo_map_uuid)?;
    let main_track_id = ctx.add_dep(raw.main_track_uuid)?;

    Ok(Arrangement {
        tempo_map_id,
        main_track_id,
        name: raw.name.to_owned(),
    })
}

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}

type ArrangementLatest<'a> = ArrangementV1<'a>;

#[derive(Debug, Serialize, Deserialize)]
struct ArrangementV1<'a> {
    tempo_map_uuid: Uuid,
    main_track_uuid: Uuid,
    name: &'a str,
}
