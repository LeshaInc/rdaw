mod hub;
mod storage;

pub use rdaw_core::Uuid;
use slotmap::Key;

pub use self::hub::{Hub, SubscribersHub};
pub use self::storage::Storage;
use crate::document;

pub trait Object: AsDynObject {
    type Id: Key
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
    pub fn get_uuid<T: Object>(&self, id: T::Id) -> Result<Uuid, document::Error> {
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
    pub fn get_id<T: Object>(&self, uuid: Uuid) -> Result<T::Id, document::Error> {
        let _ = uuid;
        todo!()
    }
}
