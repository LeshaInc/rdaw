use std::path::PathBuf;

use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct BlobId;
}

#[rdaw_macros::api_operations(BackendProtocol)]
pub trait BlobOperations {
    async fn create_internal_blob(&self, data: Vec<u8>) -> Result<BlobId>;

    async fn create_external_blob(&self, path: PathBuf) -> Result<BlobId>;
}
