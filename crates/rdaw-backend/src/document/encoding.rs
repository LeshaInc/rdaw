use serde::de::DeserializeOwned;
use serde::Serialize;

use super::{Error, Result};

pub fn serialize<T: Serialize>(version: u32, value: &T) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(128);
    vec.extend(version.to_le_bytes());
    postcard::to_extend(value, vec).map_err(|_| Error::SerializationFailed)
}

pub fn extract_version(data: &[u8]) -> Result<(u32, &[u8]), Error> {
    let version = u32::from_le_bytes(data[0..4].try_into().map_err(|_| Error::InvalidDocument)?);
    Ok((version, &data[4..]))
}

pub fn deserialize<T: DeserializeOwned>(data: &[u8]) -> Result<T, Error> {
    postcard::from_bytes(data).map_err(|_| Error::InvalidDocument)
}

#[macro_export]
macro_rules! define_version_enum {
    ( enum $Version:ident { $($v:ident = $vv:literal),* $(,)? }) => {
        define_version_enum!(@private $Version [] $($v = $vv),*);
    };

    ( @private $Version:ident [ $($v1:ident = $v1v:literal),* ] $v2:ident = $v2v:literal, $($v3:ident = $v3v:literal),* ) => {
        define_version_enum!(@private $Version [ $($v1 = $v1v,)* $v2 = $v2v ] $($v3 = $v3v),*);
    };

    ( @private $Version:ident [$($Ver:ident = $VerValue:literal),*] $Latest:ident = $LatestValue:literal) => {
        #[repr(u32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum $Version {
            $($Ver = $VerValue,)*
            $Latest = $LatestValue
        }

        impl $Version {
            pub const LATEST: $Version = $Version::$Latest;

            pub fn from_u32(v: u32) -> Result<$Version, $crate::document::Error> {
                match v {
                    $( _ if v == $Version::$Ver as u32 => Ok($Version::$Ver), )*
                    _ if v == $Version::$Latest as u32 => Ok($Version::$Latest),
                    _ => Err($crate::document::Error::UnsupportedVersion)
                }
            }

            pub fn as_u32(self) -> u32 {
                self as u32
            }
        }
    };
}
