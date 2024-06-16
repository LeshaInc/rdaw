mod blob;
mod compression;
mod database;
pub mod encoding;
mod ops;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use blake3::Hash;
use chrono::{DateTime, Utc};
use rdaw_api::Result;
use rdaw_core::Uuid;

use self::blob::{Blob, BlobChunk, BlobId};
pub use self::blob::{BlobReader, BlobWriter};
pub use self::compression::Compression;
use self::database::Database;

#[derive(Debug)]
pub struct Document {
    db: Arc<Mutex<Database>>,
    path: Option<PathBuf>,
}

impl Document {
    pub fn new() -> Result<Document> {
        let db = Database::new()?;

        Ok(Document {
            db: Arc::new(Mutex::new(db)),
            path: None,
        })
    }

    pub fn open(path: &Path) -> Result<Document> {
        let db = Database::open(path)?;
        let document = Document {
            db: Arc::new(Mutex::new(db)),
            path: Some(path.into()),
        };

        Ok(document)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn save(&self, revision: DocumentRevision) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        db.save(revision)?;
        Ok(())
    }

    pub fn save_as(&self, path: &Path, revision: DocumentRevision) -> Result<Document> {
        let db = self.db.lock().unwrap();
        let new_db = db.save_as(path, revision)?;
        Ok(Document {
            db: Arc::new(Mutex::new(new_db)),
            path: Some(path.into()),
        })
    }

    pub fn revisions(&self) -> Result<Vec<(RevisionId, DocumentRevision)>> {
        let db = self.db.lock().unwrap();
        let revisions = db.revisions()?;
        Ok(revisions)
    }

    pub fn last_revision(&self) -> Result<Option<(RevisionId, DocumentRevision)>> {
        self.revisions().map(|mut v| v.pop()) // TODO: don't get all of them
    }

    pub fn create_blob(&self, compression: Compression) -> Result<BlobWriter> {
        let id = self.db.lock().unwrap().create_blob(Blob {
            hash: None,
            total_len: 0,
            compression,
        })?;

        let writer = BlobWriter::new(self.db.clone(), id, compression);
        Ok(writer)
    }

    pub fn add_blob_dependencies(&self, target: Hash, dependencies: &[Hash]) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        db.add_blob_dependencies(target, dependencies)?;
        Ok(())
    }

    pub fn open_blob(&self, hash: Hash) -> Result<Option<BlobReader>> {
        let Some((id, blob)) = self.db.lock().unwrap().find_blob(hash)? else {
            return Ok(None);
        };

        let reader = BlobReader::new(self.db.clone(), id, blob);
        Ok(Some(reader))
    }

    pub fn remove_blob(&self, hash: Hash) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.remove_blob(hash)?;
        Ok(())
    }

    pub fn write_object(&self, uuid: Uuid, hash: Hash) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        db.write_object(uuid, hash)?;
        Ok(())
    }

    pub fn read_object(&self, uuid: Uuid) -> Result<Option<ObjectRevision>> {
        let db = self.db.lock().unwrap();
        let obj = db.read_object(uuid)?;
        Ok(obj)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RevisionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentRevision {
    pub created_at: DateTime<Utc>,
    pub time_spent_secs: u64,
    pub arrangement_uuid: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectRevision {
    pub uuid: Uuid,
    pub revision_id: RevisionId,
    pub hash: Hash,
}
