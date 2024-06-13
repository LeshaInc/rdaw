use std::path::Path;

use blake3::Hash;
use rusqlite::{Connection, OpenFlags};
use tempfile::{NamedTempFile, TempPath};

use super::{Blob, BlobChunk, BlobId, Compression, Error, Result, Revision, RevisionId};
use crate::define_version_enum;

define_version_enum! {
    enum Version {
        V1 = 1,
    }
}

#[derive(Debug)]
pub struct Database {
    db: Connection,
    _temp_path: Option<TempPath>,
}

impl Database {
    pub fn new() -> Result<Database> {
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

        db.configure()?;
        db.initialize()?;

        Ok(db)
    }

    pub fn open(path: &Path) -> Result<Database> {
        let mut db = Database {
            db: Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?,
            _temp_path: None,
        };

        db.configure()?;

        let version = db.read_version()?;
        if version != Some(Version::V1) {
            return Err(Error::InvalidDocument);
        }

        Ok(db)
    }

    fn configure(&mut self) -> Result<()> {
        self.db.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA locking_mode = EXCLUSIVE;
            ",
        )?;
        Ok(())
    }

    fn initialize(&mut self) -> Result<()> {
        self.create_schema()?;
        self.write_version(Version::LATEST)?;
        Ok(())
    }

    fn create_schema(&self) -> Result<()> {
        self.db.execute_batch(
            "
            CREATE TABLE revisions (
                id INTEGER PRIMARY KEY ASC,
                created_at TEXT,
                time_spent INTEGER
            );

            CREATE TABLE blobs (
                id INTEGER PRIMARY KEY ASC,
                hash BLOB,
                total_len INTEGER,
                compression INTEGER
            );


            CREATE TABLE blob_chunks (
                blob_id INTEGER REFERENCES blobs(id) ON DELETE CASCADE,
                offset INTEGER,
                len INTEGER,
                data BLOB
            );
            ",
        )?;
        Ok(())
    }

    fn read_version(&self) -> Result<Option<Version>> {
        let version: u32 = self
            .db
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if version == 0 {
            return Ok(None);
        }

        Version::from_u32(version).map(Some)
    }

    fn write_version(&self, version: Version) -> Result<()> {
        self.db
            .execute(&format!("PRAGMA user_version = {}", version.as_u32()), [])?;
        Ok(())
    }

    pub fn save(&self, revision: Revision) -> Result<()> {
        self.add_revision(revision)?;
        self.db.execute_batch("PRAGMA wal_checkpoint(FULL)")?;
        Ok(())
    }

    pub fn save_copy(&self, path: &Path, revision: Revision) -> Result<Database> {
        let target_dir = path
            .parent()
            .map(|v| v.to_owned())
            .unwrap_or_else(std::env::temp_dir);

        let temp_file = tempfile::Builder::new()
            .prefix(".rdaw-temp-")
            .tempfile_in(target_dir)?;

        let temp_path_str = temp_file.path().to_str().ok_or(Error::InvalidUtf8)?;
        self.db.execute("VACUUM INTO ?1", [temp_path_str])?;

        let new_db = Database::open(temp_file.path())?;
        new_db.add_revision(revision)?;
        drop(new_db);

        temp_file.persist(path).map_err(|e| Error::from(e.error))?;

        Database::open(path)
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

    pub fn write_blob(&self, blob: Blob) -> Result<BlobId> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO blobs (hash, total_len, compression) VALUES (?1, ?2, ?3) RETURNING id",
        )?;

        let hash = blob.hash.as_ref().map(|v| v.as_bytes());

        let id = stmt.query_row(
            rusqlite::params![hash, blob.total_len, blob.compression.as_u8()],
            |row| Ok(BlobId(row.get(0)?)),
        )?;

        Ok(id)
    }

    pub fn finalize_blob(&self, id: BlobId, hash: Hash, total_len: u64) -> Result<()> {
        let mut stmt = self
            .db
            .prepare_cached("UPDATE blobs SET hash = ?1, total_len = ?2 WHERE id = ?3")?;

        stmt.execute(rusqlite::params![hash.as_bytes(), total_len, id.0])?;

        Ok(())
    }

    pub fn find_blob(&self, hash: Hash) -> Result<Option<(BlobId, Blob)>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT id, total_len, compression FROM blobs WHERE hash = ?1")?;

        stmt.query([hash.as_bytes()])
            .map_err(Error::from)
            .and_then(|mut rows| {
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };

                let id = BlobId(row.get(0)?);

                let blob = Blob {
                    hash: Some(hash),
                    total_len: row.get(1)?,
                    compression: Compression::from_u8(row.get(2)?)
                        .ok_or(Error::InvalidCompressionType)?,
                };

                Ok(Some((id, blob)))
            })
    }

    pub fn remove_blob(&self, hash: Hash) -> Result<()> {
        let mut stmt = self
            .db
            .prepare_cached("DELETE FROM blobs WHERE hash = ?1")?;

        stmt.execute(rusqlite::params![hash.as_bytes()])?;

        Ok(())
    }

    pub fn remove_unsaved_blob(&self, id: BlobId) -> Result<()> {
        let mut stmt = self.db.prepare_cached("DELETE FROM blobs WHERE id = ?1")?;
        stmt.execute(rusqlite::params![id.0])?;
        Ok(())
    }

    pub fn write_blob_chunk(&self, chunk: BlobChunk<'_>) -> Result<()> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO blob_chunks (blob_id, offset, len, data) VALUES (?1, ?2, ?3, ?4)",
        )?;

        stmt.execute(rusqlite::params![
            chunk.blob_id.0,
            chunk.offset,
            chunk.len,
            chunk.data
        ])?;

        Ok(())
    }

    pub fn read_blob_chunk(
        &self,
        blob_id: BlobId,
        offset: u64,
    ) -> Result<Option<BlobChunk<'static>>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT len, data FROM blob_chunks WHERE blob_id = ?1 AND offset = ?2",
        )?;

        stmt.query(rusqlite::params![blob_id.0, offset])
            .map_err(Error::from)
            .and_then(|mut rows| {
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };

                let chunk = BlobChunk {
                    blob_id,
                    offset,
                    len: row.get(0)?,
                    data: row.get::<_, Vec<u8>>(1)?.into(),
                };

                Ok(Some(chunk))
            })
    }
}
