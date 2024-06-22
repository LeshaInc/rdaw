use std::io::Write;

use rdaw_api::asset::{AssetMetadata, AssetOperations};
use rdaw_api::document::DocumentOperations;
use rdaw_api::Result;
use rdaw_core::path::Utf8PathBuf;
use tempfile::NamedTempFile;

use crate::tests::run_test;

#[test]
fn create_external_asset() -> Result<()> {
    run_test(|client| async move {
        let data = [1, 2, 3];
        let hash = blake3::hash(&data);
        let size = data.len() as u64;

        let mut temp_file = NamedTempFile::with_prefix(".rdaw-test-")?;
        temp_file.write_all(&data)?;
        temp_file.flush()?;

        let path = Utf8PathBuf::from_path_buf(temp_file.path().into()).unwrap();

        let document_id = client.create_document().await?;
        let asset_id = client
            .create_external_asset(document_id, path.clone())
            .await?;

        let metadata = client.get_asset_metadata(asset_id).await?;
        assert_eq!(
            metadata,
            AssetMetadata {
                path: Some(path),
                hash,
                size,
            }
        );

        Ok(())
    })
}

#[test]
fn create_embedded_asset() -> Result<()> {
    run_test(|client| async move {
        let data = [1, 2, 3];
        let hash = blake3::hash(&data);
        let size = data.len() as u64;

        let document_id = client.create_document().await?;
        let asset_id = client
            .create_embedded_asset(document_id, data.into())
            .await?;

        let metadata = client.get_asset_metadata(asset_id).await?;
        assert_eq!(
            metadata,
            AssetMetadata {
                path: None,
                hash,
                size,
            }
        );

        Ok(())
    })
}
