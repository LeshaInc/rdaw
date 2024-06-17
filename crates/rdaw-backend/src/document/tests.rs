use std::io::{Read, Write};

use chrono::Utc;
use rdaw_core::path::Utf8Path;
use rdaw_core::Uuid;
use tempfile::NamedTempFile;

use super::{Compression, Document, DocumentRevision, Result, RevisionId};
use crate::document::ObjectRevision;

#[test]
fn new() -> Result<()> {
    Document::new()?;
    Ok(())
}

#[test]
fn save() -> Result<()> {
    let doc = Document::new()?;

    let revision = DocumentRevision {
        created_at: Utc::now(),
        time_spent_secs: 15,
        arrangement_uuid: Uuid::new_v4(),
    };

    doc.save(revision)?;

    Ok(())
}

#[test]
fn save_as() -> Result<()> {
    let orig = Document::new()?;

    let revision = DocumentRevision {
        created_at: Utc::now(),
        time_spent_secs: 15,
        arrangement_uuid: Uuid::new_v4(),
    };

    let copy_temp_file = NamedTempFile::with_prefix(".rdaw-test-")?;
    let copy_path = Utf8Path::from_path(copy_temp_file.path()).unwrap();
    orig.save_as(&copy_path, revision)?;

    let copy = Document::open(&copy_path)?;
    assert_eq!(copy.revisions()?, vec![(RevisionId(1), revision)]);

    Ok(())
}

#[test]
fn revisions() -> Result<()> {
    let doc = Document::new()?;

    let revision_1 = DocumentRevision {
        created_at: Utc::now(),
        time_spent_secs: 15,
        arrangement_uuid: Uuid::new_v4(),
    };
    doc.save(revision_1)?;

    let revision_2 = DocumentRevision {
        created_at: Utc::now(),
        time_spent_secs: 30,
        arrangement_uuid: Uuid::new_v4(),
    };
    doc.save(revision_2)?;

    assert_eq!(
        doc.revisions()?,
        vec![(RevisionId(1), revision_1), (RevisionId(2), revision_2)]
    );

    Ok(())
}

#[test]
fn create_blob() -> Result<()> {
    let doc = Document::new()?;

    let compression_types = [Compression::None, Compression::Zstd];

    let data_examples = [
        vec![],
        vec![1],
        vec![1, 2, 3],
        vec![0; 8192],
        vec![0; 8193],
        vec![0xFF; 23127],
    ];

    for compression in compression_types {
        for data in &data_examples {
            let mut writer = doc.create_blob(compression)?;
            writer.write_all(&data)?;
            let hash = writer.save()?;

            let mut reader = doc.open_blob(hash)?.unwrap();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;
            assert_eq!(&buf, data);

            doc.remove_blob(hash)?;
        }
    }

    Ok(())
}

#[test]
fn create_blob_with_deps() -> Result<()> {
    let doc = Document::new()?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[1])?;
    let hash_1 = writer.save()?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[2])?;
    let hash_2 = writer.save()?;

    doc.add_blob_dependencies(hash_2, &[hash_1])?;

    assert!(doc.remove_blob(hash_1).is_err());

    doc.remove_blob(hash_2)?;
    doc.remove_blob(hash_1)?;

    Ok(())
}

#[test]
fn write_object() -> Result<()> {
    let doc = Document::new()?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[1])?;
    let hash = writer.save()?;

    let uuid = Uuid::new_v4();
    doc.write_object(uuid, hash)?;

    assert_eq!(
        doc.read_object(uuid)?,
        Some(ObjectRevision {
            uuid,
            revision_id: RevisionId(0),
            hash
        })
    );

    doc.save(DocumentRevision {
        created_at: Utc::now(),
        time_spent_secs: 1,
        arrangement_uuid: Uuid::new_v4(),
    })?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[2])?;
    let hash = writer.save()?;

    doc.write_object(uuid, hash)?;

    assert_eq!(
        doc.read_object(uuid)?,
        Some(ObjectRevision {
            uuid,
            revision_id: RevisionId(1),
            hash
        })
    );

    Ok(())
}
