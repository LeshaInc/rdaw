use std::path::PathBuf;

use crate::document::DocumentId;
use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct BlobId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait BlobOperations {
    async fn create_internal_blob(&self, document_id: DocumentId, data: Vec<u8>) -> Result<BlobId>;

    async fn create_external_blob(&self, document_id: DocumentId, path: PathBuf) -> Result<BlobId>;
}
