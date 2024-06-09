use rdaw_core::Uuid;
use serde::{Deserialize, Serialize};

use super::{encoding, Result};
use crate::define_version_enum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub uuid: Uuid,
    pub main_arrangement_uuid: Option<Uuid>,
}

impl Metadata {
    pub fn new(uuid: Uuid) -> Metadata {
        Metadata {
            uuid,
            main_arrangement_uuid: None,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        encoding::serialize(Version::LATEST.as_u32(), self)
    }

    pub fn deserialize(data: &[u8]) -> Result<Metadata> {
        let (version, data) = encoding::extract_version(data)?;
        let version = Version::from_u32(version)?;
        match version {
            Version::V1 => encoding::deserialize(data),
        }
    }
}

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}
