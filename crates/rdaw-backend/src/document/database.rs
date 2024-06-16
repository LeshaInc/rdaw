use std::path::Path;

use blake3::Hash;
use rdaw_api::{bail, format_err, ErrorKind};
use rdaw_core::Uuid;
use rusqlite::{Connection, OpenFlags};
use tempfile::{NamedTempFile, TempPath};

use super::{Blob, BlobChunk, BlobId, Compression, DocumentRevision, ObjectRevision, RevisionId};
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
    next_revision: RevisionId,
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
            next_revision: RevisionId(0),
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
            next_revision: RevisionId(0),
        };

        db.configure()?;

        let Some(version) = db.read_version()? else {
            bail!(ErrorKind::Deserialization, "version field missing");
        };

        match version {
            Version::V1 => {}
        }

        db.next_revision = db.read_next_revision()?;

        Ok(db)
    }

    fn configure(&mut self) -> Result<()> {
        self.db.execute_batch(
            "
            PRAGMA foreign_keys = ON;
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
                created_at TEXT NOT NULL,
                time_spent INTEGER NOT NULL,
                arrangement_uuid BLOB NOT NULL
            );

            CREATE TABLE blobs (
                id INTEGER PRIMARY KEY ASC,
                hash BLOB,
                total_len INTEGER NOT NULL,
                compression INTEGER NOT NULL
            );

            CREATE UNIQUE INDEX blobs_hash_idx ON blobs (hash) WHERE hash IS NOT NULL;

            CREATE TABLE blob_chunks (
                blob_id INTEGER REFERENCES blobs (id) ON DELETE CASCADE,
                offset INTEGER NOT NULL,
                len INTEGER NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY (blob_id, offset)
            );

            CREATE TABLE blob_dependencies (
                parent_id INTEGER NOT NULL REFERENCES blobs (id) ON DELETE CASCADE,
                child_id INTEGER NOT NULL REFERENCES blobs (id),
                PRIMARY KEY (parent_id, child_id)
            );

            CREATE INDEX blob_dependencies_parent_idx ON blob_dependencies (parent_id);

            CREATE TABLE objects (
                uuid BLOB NOT NULL,
                revision_id INTEGER NOT NULL,
                blob_id INTEGER NOT NULL REFERENCES blobs (id),
                PRIMARY KEY (uuid, revision_id)
            );

            CREATE INDEX objects_blob_idx ON objects (blob_id);
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

        let version = Version::from_u32(version)?;
        Ok(Some(version))
    }

    fn write_version(&self, version: Version) -> Result<()> {
        self.db
            .execute(&format!("PRAGMA user_version = {}", version.as_u32()), [])?;
        Ok(())
    }

    pub fn save(&mut self, revision: DocumentRevision) -> Result<()> {
        self.save_revision(revision)?;
        self.db.execute_batch("PRAGMA wal_checkpoint(FULL)")?;
        Ok(())
    }

    pub fn save_as(&self, path: &Path, revision: DocumentRevision) -> Result<Database> {
        let target_dir = path
            .parent()
            .map(|v| v.to_owned())
            .unwrap_or_else(std::env::temp_dir);

        let temp_file = tempfile::Builder::new()
            .prefix(".rdaw-temp-")
            .tempfile_in(target_dir)?;

        let temp_path_str = temp_file
            .path()
            .to_str()
            .ok_or_else(|| format_err!(ErrorKind::InvalidUtf8, "invalid utf-8 in document path"))?;

        self.db.execute("VACUUM INTO ?1", [temp_path_str])?;

        let mut new_db = Database::open(temp_file.path())?;
        new_db.save(revision)?;
        drop(new_db);

        temp_file.persist(path).map_err(|e| Error::from(e.error))?;

        Database::open(path)
    }

    pub fn revisions(&self) -> Result<Vec<(RevisionId, DocumentRevision)>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT id, created_at, time_spent, arrangement_uuid FROM revisions")?;

        let iter = stmt.query_and_then([], |row| {
            let id = RevisionId(row.get(0)?);
            let revision = DocumentRevision {
                created_at: row.get(1)?,
                time_spent_secs: row.get(2)?,
                arrangement_uuid: row.get(3)?,
            };
            Ok((id, revision))
        })?;

        iter.collect()
    }

    pub fn next_revision(&self) -> RevisionId {
        self.next_revision
    }

    fn read_next_revision(&self) -> Result<RevisionId> {
        let mut stmt = self.db.prepare_cached("SELECT COUNT(*) FROM revisions")?;
        let id = stmt.query_row([], |row| row.get(0).map(RevisionId))?;
        Ok(id)
    }

    fn save_revision(&mut self, revision: DocumentRevision) -> Result<()> {
        let mut stmt = self.db.prepare_cached(
            "INSERT INTO revisions (created_at, time_spent, arrangement_uuid) VALUES (?1, ?2, ?3)",
        )?;

        stmt.execute(rusqlite::params![
            revision.created_at,
            revision.time_spent_secs,
            revision.arrangement_uuid
        ])?;

        self.next_revision.0 += 1;

        Ok(())
    }

    pub fn create_blob(&self, blob: Blob) -> Result<BlobId> {
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

    pub fn finalize_blob(&mut self, id: BlobId, hash: Hash, total_len: u64) -> Result<()> {
        let tx = self.db.transaction()?;

        {
            let mut stmt = tx.prepare_cached("SELECT COUNT(*) FROM blobs WHERE hash = ?1")?;
            let exists = stmt.query_row([hash.as_bytes()], |row| row.get::<_, usize>(0))? > 0;

            if exists {
                return Ok(());
            }

            let mut stmt =
                tx.prepare_cached("UPDATE blobs SET hash = ?1, total_len = ?2 WHERE id = ?3")?;
            stmt.execute(rusqlite::params![hash.as_bytes(), total_len, id.0])?;
        }

        tx.commit()?;

        Ok(())
    }

    pub fn add_blob_dependencies(&mut self, target: Hash, dependencies: &[Hash]) -> Result<()> {
        let tx = self.db.transaction()?;

        {
            let mut stmt = tx.prepare_cached("SELECT id FROM blobs WHERE hash = ?1")?;
            let id = stmt.query_row([target.as_bytes()], |row| row.get(0).map(BlobId))?;

            let mut stmt =
                tx.prepare_cached("INSERT OR IGNORE INTO blob_dependencies VALUES (?1, (SELECT id FROM blobs WHERE hash = ?2))")?;

            for dependency in dependencies {
                stmt.execute(rusqlite::params![id.0, dependency.as_bytes()])?;
            }
        }

        tx.commit()?;

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
                    compression: Compression::from_u8(row.get(2)?).ok_or_else(|| {
                        format_err!(ErrorKind::Deserialization, "invalid compression type")
                    })?,
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

    pub fn write_object(&mut self, uuid: Uuid, hash: Hash) -> Result<()> {
        let tx = self.db.transaction()?;

        {
            let mut stmt = tx.prepare_cached("SELECT id FROM blobs WHERE hash = ?1")?;
            let blob_id = stmt.query_row([hash.as_bytes()], |row| row.get(0).map(BlobId))?;

            let mut stmt = tx.prepare_cached(
                "INSERT INTO objects (uuid, revision_id, blob_id) VALUES (?1, ?2, ?3)",
            )?;
            stmt.execute(rusqlite::params![uuid, self.next_revision.0, blob_id.0])?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn read_object(&self, uuid: Uuid) -> Result<Option<ObjectRevision>> {
        let mut stmt = self.db.prepare_cached(
            "
            SELECT o.revision_id, b.hash
            FROM objects o
            JOIN blobs b ON b.id = o.blob_id
            WHERE o.uuid = ?1
            ORDER BY o.revision_id DESC
            LIMIT 1
            ",
        )?;

        stmt.query([uuid])
            .map_err(Error::from)
            .and_then(|mut rows| {
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };

                Ok(Some(ObjectRevision {
                    uuid,
                    revision_id: RevisionId(row.get(0)?),
                    hash: Hash::from_bytes(row.get(1)?),
                }))
            })
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Api(#[from] rdaw_api::Error),
}

impl From<Error> for rdaw_api::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Sql(v) => rdaw_api::Error::new(ErrorKind::Sql, v),
            Error::Io(v) => v.into(),
            Error::Api(v) => v,
        }
    }
}
