use std::fs::File;
use std::io::Write;

use blake3::Hasher;
use rdaw_api::asset::{AssetId, AssetMetadata, AssetOperations, AssetRequest, AssetResponse};
use rdaw_api::document::DocumentId;
use rdaw_api::error::ResultExt;
use rdaw_api::{bail, BackendProtocol, Error, ErrorKind, Result};
use rdaw_core::path::Utf8PathBuf;
use rdaw_rpc::Responder;
use tracing::instrument;

use super::{Asset, AssetReader, EmbeddedAsset, ExternalAsset};
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

        let file = File::open(&path).with_context(|| format!("failed to open `{path}`]"))?;

        let queue = self.queue.clone();
        self.spawn(async move {
            let mut hasher = Hasher::new();

            hasher
                .update_reader(file)
                .with_context(|| format!("failed to read `{path}`]"))?;

            let hash = hasher.finalize();
            let size = hasher.count();

            let asset = Asset::External(ExternalAsset { path, hash, size });

            queue.defer(move |this: &mut Backend| {
                let asset_id = this
                    .hub
                    .assets
                    .insert(ObjectKey::new_random(document_id), asset);

                responder.respond(Ok(asset_id))
            });

            Ok(())
        });

        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn create_embedded_asset(
        &mut self,
        responder: impl Responder<AssetId, Error>,
        document_id: DocumentId,
        data: Vec<u8>,
    ) -> Result<()> {
        let document = self.documents.get_or_err(document_id)?;
        let mut blob = document.create_blob(Compression::Zstd)?;

        let queue = self.queue.clone();
        self.spawn(async move {
            blob.write_all(&data)?;
            let hash = blob.save()?;
            let size = data.len() as u64;

            let asset = Asset::Embedded(EmbeddedAsset { hash, size });

            queue.defer(move |this: &mut Backend| {
                let asset_id = this
                    .hub
                    .assets
                    .insert(ObjectKey::new_random(document_id), asset);

                responder.respond(Ok(asset_id))
            });

            Ok(())
        });

        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_asset_metadata(&self, id: AssetId) -> Result<AssetMetadata> {
        let asset = self.hub.assets.get_or_err(id)?;
        Ok(AssetMetadata {
            path: asset.path().map(|v| v.to_path_buf()),
            hash: asset.hash(),
            size: asset.size(),
        })
    }

    pub fn open_asset(&self, id: AssetId) -> Result<AssetReader> {
        let asset = self.hub.assets.get_or_err(id)?;

        match asset {
            Asset::External(asset) => {
                let path = &asset.path;
                let file = File::open(path).with_context(|| format!("failed to open `{path}`]"))?;
                Ok(AssetReader::from_file(file))
            }
            Asset::Embedded(asset) => {
                let document_id = self.hub.assets.get_key_or_err(id)?.document_id;
                let document = self.documents.get_or_err(document_id)?;
                let Some(blob) = document.open_blob(asset.hash)? else {
                    bail!(ErrorKind::NotFound, "blob `{}` not found", asset.hash);
                };
                Ok(AssetReader::from_blob(blob))
            }
        }
    }
}
