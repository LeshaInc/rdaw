use std::io::{Read, Write};

use rdaw_api::document::DocumentId;
use rdaw_api::{bail, ErrorKind, Result};
use rdaw_core::Uuid;
use slotmap::KeyData;

use super::{Hub, Object, ObjectId, ObjectKey, ObjectType, StorageRef};
use crate::arrangement::Arrangement;
use crate::blob::Blob;
use crate::document::{Compression, DocumentStorage};
use crate::item::AudioItem;
use crate::source::AudioSource;
use crate::tempo_map::TempoMap;
use crate::track::Track;

#[derive(Debug)]
pub struct SerializationContext<'a> {
    hub: &'a Hub,
    documents: &'a DocumentStorage,
    document_id: DocumentId,
    deps: Vec<(ObjectType, Uuid, KeyData)>,
}

impl SerializationContext<'_> {
    pub fn serialize<I: ObjectId>(
        hub: &mut Hub,
        documents: &DocumentStorage,
        root_id: I,
    ) -> Result<Uuid>
    where
        I::Object: StorageRef,
    {
        let document_id = hub
            .storage::<I::Object>()
            .get_key_or_err(root_id)?
            .document_id;

        let mut ctx = SerializationContext {
            hub,
            documents,
            document_id,
            deps: Vec::new(),
        };

        let root_uuid = ctx.add_dep(root_id)?;
        ctx.serialize_loop()?;

        Ok(root_uuid)
    }

    pub fn add_dep<I: ObjectId>(&mut self, id: I) -> Result<Uuid>
    where
        I::Object: StorageRef,
    {
        let storage = self.hub.storage::<I::Object>();
        let key = storage.get_key_or_err(id)?;
        self.deps.push((I::Object::TYPE, key.uuid, id.data()));
        Ok(key.uuid)
    }

    fn serialize_loop(&mut self) -> Result<()> {
        while let Some((ty, uuid, id)) = self.deps.pop() {
            match ty {
                ObjectType::AudioItem => self.serialize_obj::<AudioItem>(uuid, id.into())?,
                ObjectType::AudioSource => self.serialize_obj::<AudioSource>(uuid, id.into())?,
                ObjectType::Track => self.serialize_obj::<Track>(uuid, id.into())?,
                ObjectType::Arrangement => self.serialize_obj::<Arrangement>(uuid, id.into())?,
                ObjectType::TempoMap => self.serialize_obj::<TempoMap>(uuid, id.into())?,
                ObjectType::Blob => self.serialize_obj::<Blob>(uuid, id.into())?,
            }
        }

        Ok(())
    }

    fn serialize_obj<T: Object + StorageRef>(&mut self, uuid: Uuid, id: T::Id) -> Result<()> {
        let storage = self.hub.storage::<T>();
        let object = storage.get_or_err(id)?;
        let data = object.serialize(self)?;

        let document = self.documents.get_or_err(self.document_id)?;

        let mut blob = document.create_blob(Compression::None)?;
        blob.write_all(&data)?;
        let hash = blob.save()?;

        document.write_object(uuid, hash)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct DeserializationContext<'a> {
    hub: &'a mut Hub,
    documents: &'a DocumentStorage,
    document_id: DocumentId,
    deps: Vec<(ObjectType, Uuid, KeyData)>,
}

impl DeserializationContext<'_> {
    pub fn deserialize<I: ObjectId>(
        hub: &mut Hub,
        documents: &DocumentStorage,
        document_id: DocumentId,
        root_uuid: Uuid,
    ) -> Result<I>
    where
        I::Object: StorageRef,
    {
        let mut ctx = DeserializationContext {
            hub,
            documents,
            document_id,
            deps: Vec::new(),
        };

        let root_id = ctx.add_dep::<I>(root_uuid)?;
        ctx.deserialize_loop()?;

        Ok(root_id)
    }

    pub fn add_dep<I: ObjectId>(&mut self, uuid: Uuid) -> Result<I>
    where
        I::Object: StorageRef,
    {
        let storage = self.hub.storage_mut::<I::Object>();
        let key = ObjectKey::new(self.document_id, uuid);

        if let Some(id) = storage.get_id(key) {
            return Ok(id);
        }

        let id = storage.prepare_insert(key);
        self.deps.push((I::Object::TYPE, uuid, id.data()));

        Ok(id)
    }

    fn deserialize_loop(&mut self) -> Result<()> {
        while let Some((ty, uuid, id)) = self.deps.pop() {
            match ty {
                ObjectType::AudioItem => self.deserialize_obj::<AudioItem>(uuid, id.into())?,
                ObjectType::AudioSource => self.deserialize_obj::<AudioSource>(uuid, id.into())?,
                ObjectType::Track => self.deserialize_obj::<Track>(uuid, id.into())?,
                ObjectType::Arrangement => self.deserialize_obj::<Arrangement>(uuid, id.into())?,
                ObjectType::TempoMap => self.deserialize_obj::<TempoMap>(uuid, id.into())?,
                ObjectType::Blob => self.deserialize_obj::<Blob>(uuid, id.into())?,
            }
        }

        Ok(())
    }

    fn deserialize_obj<T: Object + StorageRef>(&mut self, uuid: Uuid, id: T::Id) -> Result<()> {
        let document = self.documents.get_or_err(self.document_id)?;

        let Some(revision) = document.read_object(uuid)? else {
            bail!(
                ErrorKind::InvalidUuid,
                "object {uuid} doesn't exist in the document"
            );
        };

        let Some(mut blob) = document.open_blob(revision.hash)? else {
            bail!(
                ErrorKind::InvalidUuid,
                "object {uuid} doesn't have a valid blob"
            );
        };

        let mut buf = Vec::new();
        blob.read_to_end(&mut buf)?;

        let object = T::deserialize(self, &buf)?;

        let storage = self.hub.storage_mut::<T>();
        storage.finish_insert(id, object);

        Ok(())
    }
}
