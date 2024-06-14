mod encoding;
mod hub;
mod storage;

pub use rdaw_core::Uuid;

pub use self::encoding::{DeserializationContext, SerializationContext};
pub use self::hub::{Hub, StorageRef, SubscribersHub};
pub use self::storage::Storage;
use crate::document;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    AudioItem,
    AudioSource,
    Track,
    Arrangement,
    TempoMap,
    Blob,
}

pub trait Object: Sized {
    type Id: ObjectId<Object = Self>;

    const TYPE: ObjectType;

    fn serialize(&self, ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>, document::Error>;

    fn deserialize(
        ctx: &mut DeserializationContext<'_>,
        data: &[u8],
    ) -> Result<Self, document::Error>;
}

pub trait ObjectId: slotmap::Key {
    type Object: Object<Id = Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    pub uuid: Uuid,
}

impl Metadata {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Metadata {
        Metadata {
            uuid: Uuid::new_v4(),
        }
    }
}
