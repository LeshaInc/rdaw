use rdaw_api::document::DocumentId;
use rdaw_api::Result;
use rdaw_core::Uuid;

use super::{Hub, Metadata, Object, ObjectId, ObjectType, StorageRef};
use crate::arrangement::Arrangement;
use crate::blob::Blob;
use crate::item::AudioItem;
use crate::source::AudioSource;
use crate::tempo_map::TempoMap;
use crate::track::Track;

#[derive(Debug)]
pub struct SerializationContext<'a> {
    hub: &'a Hub,
    deps: Vec<(ObjectType, Uuid)>,
}

impl SerializationContext<'_> {
    pub fn serialize_graph<I: ObjectId>(hub: &Hub, root_id: I) -> Result<Uuid>
    where
        I::Object: StorageRef,
    {
        let mut ctx = SerializationContext {
            hub,
            deps: Vec::new(),
        };

        let uuid = ctx.add_dep(root_id)?;
        ctx.serialize_all()?;

        Ok(uuid)
    }

    pub fn add_dep<I: ObjectId>(&mut self, id: I) -> Result<Uuid>
    where
        I::Object: StorageRef,
    {
        let storage = self.hub.storage::<I::Object>();
        let metadata = storage.get_metadata_or_err(id)?;
        self.deps.push((I::Object::TYPE, metadata.uuid));
        Ok(metadata.uuid)
    }

    fn serialize_all(&mut self) -> Result<()> {
        while let Some((ty, uuid)) = self.deps.pop() {
            match ty {
                ObjectType::AudioItem => self.serialize::<AudioItem>(uuid)?,
                ObjectType::AudioSource => self.serialize::<AudioSource>(uuid)?,
                ObjectType::Track => self.serialize::<Track>(uuid)?,
                ObjectType::Arrangement => self.serialize::<Arrangement>(uuid)?,
                ObjectType::TempoMap => self.serialize::<TempoMap>(uuid)?,
                ObjectType::Blob => self.serialize::<Blob>(uuid)?,
            }
        }

        Ok(())
    }

    fn serialize<T: Object + StorageRef>(&mut self, uuid: Uuid) -> Result<()> {
        let storage = self.hub.storage::<T>();
        let id = storage.lookup_uuid_or_err(uuid)?;
        let object = storage.get_or_err(id)?;

        let _data = object.serialize(self)?;

        // TODO: check dirty flags
        // TODO: actually store it in the document

        Ok(())
    }
}

#[derive(Debug)]
pub struct DeserializationContext<'a> {
    hub: &'a mut Hub,
    document_id: DocumentId,
    deps: Vec<(ObjectType, Uuid)>,
}

impl DeserializationContext<'_> {
    pub fn deserialize_graph<I: ObjectId>(
        hub: &mut Hub,
        document_id: DocumentId,
        root_uuid: Uuid,
    ) -> Result<I>
    where
        I::Object: StorageRef,
    {
        let mut ctx = DeserializationContext {
            hub,
            document_id,
            deps: Vec::new(),
        };

        let root_id = ctx.add_dep::<I>(root_uuid)?;
        ctx.deserialize_all()?;

        Ok(root_id)
    }

    pub fn add_dep<I: ObjectId>(&mut self, uuid: Uuid) -> Result<I>
    where
        I::Object: StorageRef,
    {
        self.deps.push((I::Object::TYPE, uuid));

        let storage = self.hub.storage_mut::<I::Object>();
        if let Some(id) = storage.lookup_uuid(uuid) {
            return Ok(id);
        }

        let metadata = Metadata {
            uuid,
            document_id: self.document_id,
        };
        let id = storage.prepare_insert(metadata);

        Ok(id)
    }

    fn deserialize_all(&mut self) -> Result<()> {
        while let Some((ty, uuid)) = self.deps.pop() {
            match ty {
                ObjectType::AudioItem => self.deserialize::<AudioItem>(uuid)?,
                ObjectType::AudioSource => self.deserialize::<AudioSource>(uuid)?,
                ObjectType::Track => self.deserialize::<Track>(uuid)?,
                ObjectType::Arrangement => self.deserialize::<Arrangement>(uuid)?,
                ObjectType::TempoMap => self.deserialize::<TempoMap>(uuid)?,
                ObjectType::Blob => self.deserialize::<Blob>(uuid)?,
            }
        }

        Ok(())
    }

    fn deserialize<T: Object + StorageRef>(&mut self, uuid: Uuid) -> Result<()> {
        let storage = self.hub.storage::<T>();
        let id = storage.lookup_uuid_or_err(uuid)?;

        // TODO: get data from the document
        let data = &[0];
        let object = T::deserialize(self, data)?;

        let storage = self.hub.storage_mut::<T>();
        storage.finish_insert(id, object);

        Ok(())
    }
}
