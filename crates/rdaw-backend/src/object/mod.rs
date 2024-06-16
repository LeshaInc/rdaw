mod encoding;
mod hub;
mod storage;

use rdaw_api::document::DocumentId;
use rdaw_api::Result;
pub use rdaw_core::Uuid;

pub use self::encoding::{DeserializationContext, SerializationContext};
pub use self::hub::{Hub, StorageRef, SubscribersHub};
pub use self::storage::Storage;

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

    fn serialize(&self, ctx: &mut SerializationContext<'_>) -> Result<Vec<u8>>;

    fn deserialize(ctx: &mut DeserializationContext<'_>, data: &[u8]) -> Result<Self>;
}

pub trait ObjectId: slotmap::Key {
    type Object: Object<Id = Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    pub document_id: DocumentId,
    pub uuid: Uuid,
}

impl ObjectKey {
    pub fn new(document_id: DocumentId, uuid: Uuid) -> ObjectKey {
        ObjectKey { document_id, uuid }
    }

    pub fn new_random(document_id: DocumentId) -> ObjectKey {
        ObjectKey::new(document_id, Uuid::new_v4())
    }
}
