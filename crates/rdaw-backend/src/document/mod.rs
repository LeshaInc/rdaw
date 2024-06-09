mod database;
mod encoding;
mod metadata;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rdaw_core::Uuid;

use self::database::Database;
pub use self::metadata::Metadata;

#[derive(Debug)]
pub struct Document {
    metadata: Metadata,
    db: Database,
    path: Option<PathBuf>,
}

impl Document {
    pub fn new() -> Result<Document> {
        let metadata = Metadata::new(Uuid::new_v4());
        let db = Database::new(metadata)?;

        Ok(Document {
            metadata,
            db,
            path: None,
        })
    }

    pub fn open(path: &Path) -> Result<Document> {
        let (db, metadata) = Database::open(path)?;

        let document = Document {
            metadata,
            db,
            path: Some(path.into()),
        };

        Ok(document)
    }

    pub fn uuid(&self) -> Uuid {
        self.metadata.uuid
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn save(&self, revision: Revision) -> Result<()> {
        self.db.save(revision)
    }

    pub fn save_copy(&self, path: &Path, revision: Revision) -> Result<Document> {
        let db = self.db.save_copy(path, revision, self.metadata)?;
        Ok(Document {
            metadata: self.metadata,
            db,
            path: Some(path.into()),
        })
    }

    pub fn revisions(&self) -> Result<Vec<(RevisionId, Revision)>> {
        self.db.revisions()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid datetime")]
    InvalidDateTime,
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("invalid document")]
    InvalidDocument,
    #[error("invalid utf8")]
    InvalidUtf8,
    #[error("serialization failed")]
    SerializationFailed,
    #[error("database error: {error}")]
    Database {
        #[source]
        #[from]
        error: rusqlite::Error,
    },
    #[error("io error: {error}")]
    Io {
        #[source]
        #[from]
        error: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RevisionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision {
    pub created_at: DateTime<Utc>,
    pub time_spent_secs: u64,
}
