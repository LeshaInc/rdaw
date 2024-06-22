use std::io::Write;

use rdaw_api::asset::AssetOperations;
use rdaw_api::document::DocumentOperations;
use rdaw_api::Result;
use rdaw_core::path::Utf8PathBuf;
use tempfile::NamedTempFile;

use crate::tests::run_test;

#[test]
fn create_external_asset() -> Result<()> {
    run_test(|client| async move {
        let mut temp_file = NamedTempFile::with_prefix(".rdaw-test-")?;
        temp_file.write_all(&[1, 2, 3])?;
        temp_file.flush()?;

        let path = Utf8PathBuf::from_path_buf(temp_file.path().into()).unwrap();

        let document_id = client.create_document().await?;
        let _asset_id = client.create_external_asset(document_id, path).await?;

        Ok(())
    })
}

#[test]
fn create_embedded_asset() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;
        let _asset_id = client
            .create_embedded_asset(document_id, vec![1, 2, 3])
            .await?;

        Ok(())
    })
}
