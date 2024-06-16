use chrono::Utc;
use rdaw_api::arrangement::ArrangementId;
use rdaw_api::document::{DocumentId, DocumentOperations, DocumentRequest, DocumentResponse};
use rdaw_api::{format_err, BackendProtocol, ErrorKind, Result};
use tracing::instrument;

use super::{Document, DocumentRevision};
use crate::object::{DeserializationContext, SerializationContext};
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = DocumentOperations)]
impl Backend {
    #[instrument(skip_all, err)]
    #[handler]
    pub fn create_document(&mut self) -> Result<DocumentId> {
        let document = Document::new()?;

        let document_id = self.hub.documents.insert(document);

        let arrangement_id = self.create_arrangement(document_id)?;
        let arrangement_uuid = self
            .hub
            .arrangements
            .get_metadata_or_err(arrangement_id)?
            .uuid;

        let document = &self.hub.documents[document_id];

        document.save(DocumentRevision {
            created_at: Utc::now(),
            time_spent_secs: 0,
            arrangement_uuid,
        })?;

        Ok(document_id)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn open_document(&mut self, path: String) -> Result<DocumentId> {
        let document = Document::open(path.as_ref())?;

        let (_, last_revision) = document
            .last_revision()?
            .ok_or_else(|| format_err!(ErrorKind::Other, "document doesn't have any revision"))?;

        let document_id = self.hub.documents.insert(document);

        DeserializationContext::deserialize::<ArrangementId>(
            &mut self.hub,
            document_id,
            last_revision.arrangement_uuid,
        )?;

        Ok(document_id)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn save_document(&mut self, id: DocumentId) -> Result<()> {
        let document = self
            .hub
            .documents
            .get(id)
            .ok_or_else(|| format_err!(ErrorKind::InvalidId, "invalid {id:?}"))?;

        let (_, last_revision) = document
            .last_revision()?
            .ok_or_else(|| format_err!(ErrorKind::Other, "document doesn't have any revision"))?;

        let arrangement_id = self
            .hub
            .arrangements
            .lookup_uuid_or_err(last_revision.arrangement_uuid)?;

        SerializationContext::serialize(&mut self.hub, arrangement_id)?;

        let document = &self.hub.documents[id];
        document.save(DocumentRevision {
            created_at: Utc::now(),
            time_spent_secs: 0,
            arrangement_uuid: last_revision.arrangement_uuid,
        })?;

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn save_document_as(&mut self, id: DocumentId, path: String) -> Result<()> {
        let document = self
            .hub
            .documents
            .get(id)
            .ok_or_else(|| format_err!(ErrorKind::InvalidId, "invalid {id:?}"))?;

        let (_, last_revision) = document
            .last_revision()?
            .ok_or_else(|| format_err!(ErrorKind::Other, "document doesn't have any revision"))?;

        let arrangement_id = self
            .hub
            .arrangements
            .lookup_uuid_or_err(last_revision.arrangement_uuid)?;

        SerializationContext::serialize(&mut self.hub, arrangement_id)?;

        let document = &self.hub.documents[id];
        let new_document = document.save_as(
            path.as_ref(),
            DocumentRevision {
                created_at: Utc::now(),
                time_spent_secs: 0,
                arrangement_uuid: last_revision.arrangement_uuid,
            },
        )?;

        self.hub.documents[id] = new_document;

        Ok(())
    }

    #[instrument(skip_all, err)]
    #[handler]
    fn get_document_arrangement(&self, id: DocumentId) -> Result<ArrangementId> {
        let document = self
            .hub
            .documents
            .get(id)
            .ok_or_else(|| format_err!(ErrorKind::InvalidId, "invalid {id:?}"))?;

        let (_, last_revision) = document
            .last_revision()?
            .ok_or_else(|| format_err!(ErrorKind::Other, "document doesn't have any revision"))?;

        self.hub
            .arrangements
            .lookup_uuid_or_err(last_revision.arrangement_uuid)
    }
}
