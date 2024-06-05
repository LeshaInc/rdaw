use chrono::Utc;
use tempfile::NamedTempFile;

use super::{Document, Result, Revision, RevisionId};

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
fn save_copy() -> Result<()> {
    let orig = Document::new()?;

    let revision = Revision {
        created_at: Utc::now(),
        time_spent_secs: 15,
    };

    let copy_path = NamedTempFile::with_prefix(".rdaw-test-")?.into_temp_path();
    orig.save_copy(&copy_path, revision)?;

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
