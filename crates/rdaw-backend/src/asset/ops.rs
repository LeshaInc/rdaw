use std::io::Write;

use blake3::Hasher;
use rdaw_api::asset::{AssetId, AssetOperations, AssetRequest, AssetResponse};
use rdaw_api::document::DocumentId;
use rdaw_api::{BackendProtocol, Error, Result};
use rdaw_core::path::Utf8PathBuf;
use rdaw_rpc::Responder;
use tracing::instrument;

use super::{Asset, EmbeddedAsset, ExternalAsset};
use crate::document::Compression;
use crate::object::ObjectKey;
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = AssetOperations)]
impl Backend {
    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn create_external_asset(
        &mut self,
        responder: impl Responder<AssetId, Error>,
        document_id: DocumentId,
        path: Utf8PathBuf,
    ) -> Result<()> {
        self.documents.ensure_has(document_id)?;

        let queue = self.queue.clone();
        self.thread_pool.spawn_ok(async move {
            let mut hasher = Hasher::new();
            hasher.update_mmap(&path).unwrap();
            let hash = hasher.finalize();

            queue.defer(move |this: &mut Backend| {
                let asset = Asset::External(ExternalAsset { path, hash });

                let asset_id = this
                    .hub
                    .assets
                    .insert(ObjectKey::new_random(document_id), asset);

                responder.respond(Ok(asset_id))
            })
        });

        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn create_embedded_asset(
        &mut self,
        document_id: DocumentId,
        data: Vec<u8>,
    ) -> Result<AssetId> {
        let document = self.documents.get_or_err(document_id)?;

        let mut blob = document.create_blob(Compression::Zstd)?;
        blob.write_all(&data)?;
        let hash = blob.save()?;

        let asset = Asset::Embedded(EmbeddedAsset { hash });
        let asset_id = self
            .hub
            .assets
            .insert(ObjectKey::new_random(document_id), asset);

        Ok(asset_id)
    }
}
