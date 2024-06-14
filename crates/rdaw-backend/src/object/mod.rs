mod encoding;
mod hub;
mod storage;

use rdaw_api::document::DocumentId;
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
    pub document_id: DocumentId,
}

impl Metadata {
    pub fn new(document_id: DocumentId) -> Metadata {
        Metadata {
            uuid: Uuid::new_v4(),
            document_id,
        }
    }
}
