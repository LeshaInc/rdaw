use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use tempfile::{NamedTempFile, TempPath};

use super::{Error, Metadata, Result, Revision, RevisionId};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V1 = 1,
}

impl Version {
    pub const LATEST: Version = Version::V1;
}

#[derive(Debug)]
pub struct Database {
    db: Connection,
    _temp_path: Option<TempPath>,
}

impl Database {
    pub fn new(metadata: Metadata) -> Result<Database> {
        let temp_path = NamedTempFile::with_prefix(".rdaw-unsaved-")?.into_temp_path();

        let mut db = Database {
            db: Connection::open_with_flags(
                &temp_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
            _temp_path: Some(temp_path),
        };

        db.initialize(metadata)?;

        Ok(db)
    }

    pub fn open(path: &Path) -> Result<(Database, Metadata)> {
        let db = Database {
            db: Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
            _temp_path: None,
        };

        let version = db.read_version()?;
        if version != Some(Version::V1) {
            return Err(Error::InvalidDocument);
        }

        let metadata = db.read_metadata()?;
        Ok((db, metadata))
    }

    fn initialize(&mut self, metadata: Metadata) -> Result<()> {
        self.create_schema()?;
        self.write_version(Version::LATEST)?;
        self.write_metadata(metadata)?;
        Ok(())
    }

    fn create_schema(&self) -> Result<()> {
        self.db.execute_batch(
            "
            CREATE TABLE metadata (
                uuid BLOB
            );

            CREATE TABLE revisions (
                id INTEGER PRIMARY KEY ASC,
                created_at TEXT,
                time_spent INTEGER
            );
        ",
        )?;
        Ok(())
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

    fn write_version(&self, version: Version) -> Result<()> {
        self.db
            .execute(&format!("PRAGMA user_version = {}", version as u32), [])?;
        Ok(())
    }

    fn read_metadata(&self) -> Result<Metadata> {
        Ok(self.db.query_row("SELECT uuid FROM metadata", [], |row| {
            Ok(Metadata { uuid: row.get(0)? })
        })?)
    }

    fn write_metadata(&self, metadata: Metadata) -> Result<()> {
        self.db
            .execute("INSERT INTO metadata (uuid) VALUES (?1)", [metadata.uuid])?;
        Ok(())
    }

    pub fn save(&self, revision: Revision) -> Result<()> {
        self.db.cache_flush()?;
        self.add_revision(revision)?;
        Ok(())
    }

    pub fn save_copy(
        &self,
        path: &Path,
        revision: Revision,
        metadata: Metadata,
    ) -> Result<Database> {
        let target_dir = path
            .parent()
            .map(|v| v.to_owned())
            .unwrap_or_else(std::env::temp_dir);

        let temp_file = tempfile::Builder::new()
            .prefix(".rdaw-temp-")
            .tempfile_in(target_dir)?;

        let temp_path_str = temp_file.path().to_str().ok_or(Error::InvalidUtf8)?;
        self.db.execute("VACUUM INTO ?1", [temp_path_str])?;

        let (new_db, _) = Database::open(temp_file.path())?;
        new_db.add_revision(revision)?;
        new_db.write_metadata(metadata)?;
        drop(new_db);

        temp_file.persist(path).map_err(|e| Error::from(e.error))?;

        let (new_db, _) = Database::open(path)?;
        Ok(new_db)
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
