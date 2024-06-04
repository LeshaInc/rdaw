#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use tempfile::{NamedTempFile, TempPath};

#[derive(Debug)]
pub struct Document {
    path: Option<PathBuf>,
    _temp_path: Option<TempPath>,
    db: Connection,
}

impl Document {
    pub fn new() -> Result<Document> {
        let temp_path = NamedTempFile::with_prefix("rdaw-unsaved-")?.into_temp_path();
        let db = Connection::open(&temp_path)?;

        let mut document = Document {
            path: None,
            _temp_path: Some(temp_path),
            db,
        };

        document.create_schema()?;

        Ok(document)
    }

    pub fn open(path: &Path) -> Result<Document> {
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let document = Document {
            path: Some(path.into()),
            _temp_path: None,
            db,
        };

        let version = document.read_version()?;
        if version != Some(Version::V1) {
            return Err(Error::InvalidDocument);
        }

        Ok(document)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn read_version(&self) -> Result<Option<Version>> {
        let version: u32 = self
            .db
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;

        match version {
            0 => Ok(None),
            1 => Ok(Some(Version::V1)),
            _ => Err(Error::UnsupportedVersion),
        }
    }

    fn create_schema(&mut self) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Exclusive)?;

        tx.execute(
            &format!("PRAGMA user_version = {}", Version::LATEST as u32),
            [],
        )?;

        let sql = "
            CREATE TABLE revisions (
                id INTEGER PRIMARY KEY ASC,
                created_at TEXT,
                time_spent INTEGER
            );
        ";

        tx.execute_batch(sql)?;
        tx.commit()?;

        Ok(())
    }

    pub fn save(&self, revision: Revision) -> Result<()> {
        self.db.cache_flush()?;
        self.add_revision(revision)?;
        Ok(())
    }

    pub fn save_as(&self, path: &Path, revision: Revision) -> Result<Document> {
        let path_str = path.to_str().ok_or(Error::InvalidUtf8)?;

        self.db.execute("VACUUM INTO ?1", [path_str])?;

        let new_db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let new_doc = Document {
            path: Some(path.into()),
            _temp_path: None,
            db: new_db,
        };

        new_doc.add_revision(revision)?;

        Ok(new_doc)
    }

    pub fn revisions(&self) -> Result<Vec<(RevisionId, Revision)>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT id, created_at, time_spent FROM revisions")?;

        let iter = stmt.query_and_then([], |row| {
            let id = RevisionId(row.get(0)?);
            let revision = Revision {
                created_at: row.get(1)?,
                time_spent_secs: row.get(2)?,
            };
            Ok((id, revision))
        })?;

        iter.collect()
    }

    fn add_revision(&self, revision: Revision) -> Result<()> {
        let mut stmt = self
            .db
            .prepare_cached("INSERT INTO revisions (created_at, time_spent) VALUES (?1, ?2)")?;

        stmt.execute(rusqlite::params![
            revision.created_at,
            revision.time_spent_secs
        ])?;

        Ok(())
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V1 = 1,
}

impl Version {
    pub const LATEST: Version = Version::V1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RevisionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision {
    pub created_at: DateTime<Utc>,
    pub time_spent_secs: u64,
}
