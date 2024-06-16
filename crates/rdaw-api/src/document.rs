use crate::arrangement::ArrangementId;
use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct DocumentId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait DocumentOperations {
    async fn create_document(&self) -> Result<DocumentId>;

    async fn open_document(&self, path: String) -> Result<DocumentId>;

    async fn save_document(&self, id: DocumentId) -> Result<()>;

    async fn save_document_as(&self, id: DocumentId, path: String) -> Result<()>;

    async fn get_document_arrangement(&self, id: DocumentId) -> Result<ArrangementId>;
}
