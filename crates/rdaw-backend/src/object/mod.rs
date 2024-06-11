mod hub;
mod storage;

pub use rdaw_core::Uuid;

pub use self::hub::{Hub, SubscribersHub};
pub use self::storage::Storage;
use crate::document;

pub trait Object: AsDynObject {
    type Id: ObjectId
    where
        Self: Sized;

    fn trace(&self, hub: &Hub, callback: &mut dyn FnMut(&dyn Object)) {
        let _ = (hub, callback);
    }

    fn serialize(&self, ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error>;

    fn deserialize(ctx: &DeserializationContext<'_>, data: &[u8]) -> Result<Self, document::Error>
    where
        Self: Sized;
}

pub trait ObjectId: slotmap::Key {
    type Object: Object;
}

pub trait AsDynObject {
    fn as_dyn_object(&self) -> &dyn Object;
}

impl<T: Object> AsDynObject for T {
    fn as_dyn_object(&self) -> &dyn Object {
        self
    }
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

#[derive(Debug)]
pub struct SerializationContext<'a> {
    #[allow(dead_code)]
    hub: &'a Hub,
}

impl SerializationContext<'_> {
    pub fn get_uuid<I: ObjectId>(&self, id: I) -> Result<Uuid, document::Error> {
        let _ = id;
        todo!()
    }
}

#[derive(Debug)]
pub struct DeserializationContext<'a> {
    #[allow(dead_code)]
    hub: &'a mut Hub,
}

impl DeserializationContext<'_> {
    pub fn get_id<I: ObjectId>(&self, uuid: Uuid) -> Result<I, document::Error> {
        let _ = uuid;
        todo!()
    }
}
