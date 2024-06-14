use std::io::{Read, Write};

use chrono::Utc;
use rdaw_core::Uuid;
use tempfile::NamedTempFile;

use super::{Compression, Document, Result, Revision, RevisionId};
use crate::document::ObjectRevision;

#[test]
fn new() -> Result<()> {
    Document::new()?;
    Ok(())
}

#[test]
fn save() -> Result<()> {
    let doc = Document::new()?;

    let revision = Revision {
        created_at: Utc::now(),
        time_spent_secs: 15,
    };

    doc.save(revision)?;

    Ok(())
}

#[test]
fn save_as() -> Result<()> {
    let orig = Document::new()?;

    let revision = Revision {
        created_at: Utc::now(),
        time_spent_secs: 15,
    };

    let copy_path = NamedTempFile::with_prefix(".rdaw-test-")?.into_temp_path();
    orig.save_as(&copy_path, revision)?;

    let copy = Document::open(&copy_path)?;
    assert_eq!(copy.revisions()?, vec![(RevisionId(1), revision)]);

    Ok(())
}

#[test]
fn revisions() -> Result<()> {
    let doc = Document::new()?;

    let revision_1 = Revision {
        created_at: Utc::now(),
        time_spent_secs: 15,
    };
    doc.save(revision_1)?;

    let revision_2 = Revision {
        created_at: Utc::now(),
        time_spent_secs: 30,
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
            let hash = writer.save(&[])?;

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
    let blob1 = writer.save(&[])?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[2])?;
    let blob2 = writer.save(&[])?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[1, 2])?;
    let blob_uses_1 = writer.save(&[blob1])?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[1, 2])?;
    assert!(writer.save(&[blob1, blob2]).is_err());

    assert!(doc.remove_blob(blob1).is_err());

    doc.remove_blob(blob_uses_1)?;
    doc.remove_blob(blob1)?;
    doc.remove_blob(blob2)?;

    Ok(())
}

#[test]
fn write_object() -> Result<()> {
    let doc = Document::new()?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[1])?;
    let hash = writer.save(&[])?;

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

    doc.save(Revision {
        created_at: Utc::now(),
        time_spent_secs: 1,
    })?;

    let mut writer = doc.create_blob(Compression::None)?;
    writer.write_all(&[2])?;
    let hash = writer.save(&[])?;

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
