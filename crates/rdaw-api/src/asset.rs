use rdaw_core::path::Utf8PathBuf;

use crate::document::DocumentId;
use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct AssetId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait AssetOperations {
    async fn create_external_asset(
        &self,
        document_id: DocumentId,
        path: Utf8PathBuf,
    ) -> Result<AssetId>;

    async fn create_embedded_asset(
        &self,
        document_id: DocumentId,
        data: Vec<u8>,
    ) -> Result<AssetId>;
}
