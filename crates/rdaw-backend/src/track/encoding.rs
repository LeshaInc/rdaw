use rdaw_api::item::{ItemId, ItemKind};
use rdaw_api::time::Time;
use rdaw_api::track::TrackItem;
use rdaw_api::Result;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use super::{Track, TrackLinks};
use crate::define_version_enum;
use crate::document::encoding;
use crate::object::{DeserializationContext, SerializationContext, Uuid};

pub fn serialize(ctx: &mut SerializationContext<'_>, track: &Track) -> Result<Vec<u8>> {
    let children = track
        .links
        .children
        .iter()
        .map(|&id| ctx.add_dep(id))
        .collect::<Result<Vec<_>>>()?;

    let items = track
        .items
        .iter()
        .map(|(_, item)| {
            let (kind, uuid) = match item.inner {
                ItemId::Audio(id) => (ItemKind::Audio, ctx.add_dep(id)?),
            };

            Ok(TrackItemLatest {
                kind,
                uuid,
                start: item.start,
                duration: item.duration,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let raw = TrackLatest {
        name: &track.name,
        children,
        items,
    };

    encoding::serialize(Version::LATEST.as_u32(), &raw)
}

pub fn deserialize(ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Track> {
    let (version, data) = encoding::extract_version(data)?;
    let raw = match Version::from_u32(version)? {
        Version::V1 => encoding::deserialize::<TrackV1>(data)?,
    };

    let name = raw.name.to_owned();

    let children = raw
        .children
        .into_iter()
        .map(|uuid| ctx.add_dep(uuid))
        .collect::<Result<Vec<_>>>()?;

    let mut items = SlotMap::with_capacity_and_key(raw.items.len());

    for item in raw.items {
        let inner = match item.kind {
            ItemKind::Audio => ItemId::Audio(ctx.add_dep(item.uuid)?),
        };

        items.insert(TrackItem {
            inner,
            start: item.start,
            duration: item.duration,
        });
    }

    Ok(Track {
        name,
        links: TrackLinks {
            children,
            ..Default::default()
        },
        items,
    })
}

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}

type TrackLatest<'a> = TrackV1<'a>;
type TrackItemLatest = TrackItemV1;

#[derive(Debug, Serialize, Deserialize)]
struct TrackV1<'a> {
    name: &'a str,
    children: Vec<Uuid>,
    items: Vec<TrackItemV1>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrackItemV1 {
    kind: ItemKind,
    uuid: Uuid,
    start: Time,
    duration: Time,
}
