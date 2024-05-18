use std::fmt;
use std::path::PathBuf;

use blake3::Hash;
use rdaw_api::{BlobOperations, Error, Result};
use rdaw_core::collections::HashMap;
use rdaw_object::{Blob, BlobId};
use tracing::instrument;

use crate::{Backend, BackendHandle};

crate::dispatch::define_dispatch_ops! {
    pub enum BlobOperation;

    impl Backend {
        pub fn dispatch_blob_operation;
    }

    impl BlobOperations for BackendHandle;

    CreateInternalBlob => create_internal_blob(
        data: Vec<u8>,
    ) -> Result<BlobId>;

    CreateExternalBlob => create_external_blob(
        path: PathBuf,
    ) -> Result<BlobId>;
}

impl Backend {
    #[instrument(skip_all, err)]
    pub async fn create_internal_blob(&mut self, data: Vec<u8>) -> Result<BlobId> {
        let hash = blake3::hash(&data);
        self.blob_cache.insert(hash, data);

        let blob = Blob::new_internal(hash);
        let id = self.hub.blobs.insert(blob);

        Ok(id)
    }

    #[instrument(skip_all, err)]
    pub async fn create_external_blob(&mut self, path: PathBuf) -> Result<BlobId> {
        let data = std::fs::read(&path).map_err(|error| Error::Filesystem {
            error,
            path: path.clone(),
        })?;

        let hash = blake3::hash(&data);
        self.blob_cache.insert(hash, data);

        let blob = Blob::new_external(hash, path);
        let id = self.hub.blobs.insert(blob);

        Ok(id)
    }
}

#[derive(Default)]
pub struct BlobCache {
    map: HashMap<Hash, Vec<u8>>,
}

impl BlobCache {
    pub fn insert(&mut self, hash: Hash, data: Vec<u8>) {
        self.map.insert(hash, data);
    }
}

impl fmt::Debug for BlobCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlobCache").finish_non_exhaustive()
    }
}
