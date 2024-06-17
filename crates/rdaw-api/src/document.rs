use rdaw_core::path::Utf8PathBuf;

use crate::arrangement::ArrangementId;
use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct DocumentId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait DocumentOperations {
    async fn create_document(&self) -> Result<DocumentId>;

    async fn open_document(&self, path: Utf8PathBuf) -> Result<DocumentId>;

    async fn save_document(&self, id: DocumentId) -> Result<()>;

    async fn save_document_as(&self, id: DocumentId, path: Utf8PathBuf) -> Result<()>;

    async fn get_document_arrangement(&self, id: DocumentId) -> Result<ArrangementId>;
}
