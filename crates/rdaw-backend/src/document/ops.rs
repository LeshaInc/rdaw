use rdaw_api::document::{DocumentId, DocumentOperations, DocumentRequest, DocumentResponse};
use rdaw_api::{BackendProtocol, Result};
use tracing::instrument;

use super::Document;
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = DocumentOperations)]
impl Backend {
    #[instrument(skip_all, err)]
    #[handler]
    pub fn create_document(&mut self) -> Result<DocumentId> {
        let document = Document::new().unwrap(); // TODO
        let id = self.hub.documents.insert(document);
        Ok(id)
    }
}
