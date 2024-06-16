use std::io::{Read, Write};

use rdaw_api::document::DocumentId;
use rdaw_api::{bail, format_err, ErrorKind, Result};
use rdaw_core::Uuid;
use slotmap::SlotMap;

use super::{Hub, Metadata, Object, ObjectId, ObjectType, StorageRef};
use crate::arrangement::Arrangement;
use crate::blob::Blob;
use crate::document::{Compression, Document};
use crate::item::AudioItem;
use crate::source::AudioSource;
use crate::tempo_map::TempoMap;
use crate::track::Track;

#[derive(Debug)]
pub struct SerializationContext<'a> {
    documents: SlotMap<DocumentId, Document>,
    hub: &'a Hub,
    deps: Vec<(ObjectType, Uuid)>,
}

impl SerializationContext<'_> {
    pub fn serialize<I: ObjectId>(hub: &mut Hub, root_id: I) -> Result<Uuid>
    where
        I::Object: StorageRef,
    {
        let mut ctx = SerializationContext {
            documents: std::mem::take(&mut hub.documents),
            hub,
            deps: Vec::new(),
        };

        let uuid = ctx.add_dep(root_id)?;

        ctx.serialize_loop()?;

        std::mem::swap(&mut ctx.documents, &mut hub.documents);

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

    fn serialize_loop(&mut self) -> Result<()> {
        while let Some((ty, uuid)) = self.deps.pop() {
            dbg!(ty, uuid);
            match ty {
                ObjectType::AudioItem => self.serialize_obj::<AudioItem>(uuid)?,
                ObjectType::AudioSource => self.serialize_obj::<AudioSource>(uuid)?,
                ObjectType::Track => self.serialize_obj::<Track>(uuid)?,
                ObjectType::Arrangement => self.serialize_obj::<Arrangement>(uuid)?,
                ObjectType::TempoMap => self.serialize_obj::<TempoMap>(uuid)?,
                ObjectType::Blob => self.serialize_obj::<Blob>(uuid)?,
            }
        }

        Ok(())
    }

    fn serialize_obj<T: Object + StorageRef>(&mut self, uuid: Uuid) -> Result<()> {
        let storage = self.hub.storage::<T>();
        let id = storage.lookup_uuid_or_err(uuid)?;

        // if !storage.is_dirty(id) {
        //     return Ok(());
        // }

        let object = storage.get_or_err(id)?;
        let data = object.serialize(self)?;

        let document_id = storage.get_metadata_or_err(id)?.document_id;
        let document = self
            .documents
            .get(document_id)
            .ok_or_else(|| format_err!(ErrorKind::InvalidId, "invalid {document_id:?}"))?;

        let mut blob = document.create_blob(Compression::None)?;
        blob.write_all(&data)?;
        let hash = blob.save()?;

        document.write_object(uuid, hash)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct DeserializationContext<'a> {
    documents: SlotMap<DocumentId, Document>,
    document_id: DocumentId,
    hub: &'a mut Hub,
    deps: Vec<(ObjectType, Uuid)>,
}

impl DeserializationContext<'_> {
    pub fn deserialize<I: ObjectId>(
        hub: &mut Hub,
        document_id: DocumentId,
        root_uuid: Uuid,
    ) -> Result<I>
    where
        I::Object: StorageRef,
    {
        let mut ctx = DeserializationContext {
            documents: std::mem::take(&mut hub.documents),
            document_id,
            hub,
            deps: Vec::new(),
        };

        let root_id = ctx.add_dep::<I>(root_uuid)?;
        ctx.deserialize_loop()?;

        std::mem::swap(&mut ctx.documents, &mut hub.documents);

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

    fn deserialize_loop(&mut self) -> Result<()> {
        while let Some((ty, uuid)) = self.deps.pop() {
            match ty {
                ObjectType::AudioItem => self.deserialize_obj::<AudioItem>(uuid)?,
                ObjectType::AudioSource => self.deserialize_obj::<AudioSource>(uuid)?,
                ObjectType::Track => self.deserialize_obj::<Track>(uuid)?,
                ObjectType::Arrangement => self.deserialize_obj::<Arrangement>(uuid)?,
                ObjectType::TempoMap => self.deserialize_obj::<TempoMap>(uuid)?,
                ObjectType::Blob => self.deserialize_obj::<Blob>(uuid)?,
            }
        }

        Ok(())
    }

    fn deserialize_obj<T: Object + StorageRef>(&mut self, uuid: Uuid) -> Result<()> {
        let storage = self.hub.storage::<T>();
        let id = storage.lookup_uuid_or_err(uuid)?;

        let document = self
            .documents
            .get(self.document_id)
            .ok_or_else(|| format_err!(ErrorKind::InvalidId, "invalid {:?}", self.document_id))?;

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
