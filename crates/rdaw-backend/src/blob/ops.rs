use std::path::PathBuf;

use rdaw_api::blob::{BlobId, BlobOperations, BlobRequest, BlobResponse};
use rdaw_api::document::DocumentId;
use rdaw_api::{BackendProtocol, Error, Result};
use tracing::instrument;

use super::Blob;
use crate::object::Metadata;
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = BlobOperations)]
impl Backend {
    #[instrument(skip_all, err)]
    #[handler]
    pub fn create_internal_blob(
        &mut self,
        document_id: DocumentId,
        data: Vec<u8>,
    ) -> Result<BlobId> {
        let hash = blake3::hash(&data);
        self.blob_cache.insert(hash, data);

        let blob = Blob::Internal { hash };
        let id = self.hub.blobs.insert(Metadata::new(document_id), blob);

        Ok(id)
    }

    #[instrument(skip_all, err)]
    #[handler]
    pub fn create_external_blob(
        &mut self,
        document_id: DocumentId,
        path: PathBuf,
    ) -> Result<BlobId> {
        let data = std::fs::read(&path).map_err(|e| {
            Error::from(e).context(format!("failed to read file {}", path.display()))
        })?;

        let hash = blake3::hash(&data);
        self.blob_cache.insert(hash, data);

        let blob = Blob::External { hash, path };
        let id = self.hub.blobs.insert(Metadata::new(document_id), blob);

        Ok(id)
    }
}
