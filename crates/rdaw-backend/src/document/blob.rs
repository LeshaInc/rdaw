use std::borrow::Cow;
use std::io;
use std::sync::{Arc, Mutex};

use blake3::{Hash, Hasher};

use super::database::Database;
use super::Compression;

const CHUNK_SIZE: usize = 8192;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BlobId(pub i64);

#[derive(Debug)]
pub struct Blob {
    pub hash: Option<Hash>,
    pub total_len: Option<u64>,
    pub compression: Compression,
}

#[derive(Debug)]
pub struct BlobChunk<'a> {
    pub blob_id: BlobId,
    pub offset: u64,
    pub len: u64,
    pub data: Cow<'a, [u8]>,
}

#[derive(Debug)]
pub struct BlobWriter {
    db: Arc<Mutex<Database>>,
    blob_id: BlobId,
    hasher: Hasher,
    compression: Compression,
    offset: u64,
    buffer: Vec<u8>,
}

impl BlobWriter {
    pub(super) fn new(
        db: Arc<Mutex<Database>>,
        blob_id: BlobId,
        compression: Compression,
    ) -> BlobWriter {
        BlobWriter {
            db,
            blob_id,
            hasher: Hasher::new(),
            compression,
            offset: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE),
        }
    }

    fn flush_chunks(&mut self, close: bool) -> io::Result<()> {
        while !self.buffer.is_empty() && (self.buffer.len() >= CHUNK_SIZE || close) {
            let chunk_len = CHUNK_SIZE.min(self.buffer.len());

            let data = self.compression.compress(&self.buffer[..chunk_len])?;

            let chunk = BlobChunk {
                blob_id: self.blob_id,
                offset: self.offset,
                len: chunk_len as u64,
                data,
            };

            let db = self.db.lock().unwrap();
            db.write_blob_chunk(chunk).map_err(io::Error::other)?;

            self.buffer.drain(..chunk_len);
            self.offset += chunk_len as u64;
        }

        Ok(())
    }

    fn close_inner(&mut self) -> io::Result<Hash> {
        let hash = self.hasher.finalize();

        self.flush_chunks(true)?;

        let db = self.db.lock().unwrap();
        db.finalize_blob(self.blob_id, hash, self.offset)
            .map_err(io::Error::other)?;

        Ok(hash)
    }

    pub fn close(mut self) -> io::Result<Hash> {
        self.close_inner()
    }
}

impl io::Write for BlobWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.hasher.update(buf);

        for chunk in buf.chunks(CHUNK_SIZE) {
            self.buffer.extend_from_slice(chunk);
            self.flush_chunks(false)?;
        }

        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write(buf)?;
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for BlobWriter {
    fn drop(&mut self) {
        if let Err(error) = self.close_inner() {
            tracing::error!(?error, "Failed to save blob")
        }
    }
}

#[derive(Debug)]
pub struct BlobReader {
    db: Arc<Mutex<Database>>,
    id: BlobId,
    blob: Blob,
    offset: u64,
    buffer: Vec<u8>,
}

impl BlobReader {
    pub(super) fn new(db: Arc<Mutex<Database>>, id: BlobId, blob: Blob) -> BlobReader {
        BlobReader {
            db,
            id,
            blob,
            offset: 0,
            buffer: Vec::with_capacity(CHUNK_SIZE),
        }
    }
}

impl io::Read for BlobReader {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let existing = buf.len().min(self.buffer.len());
        buf[..existing].copy_from_slice(&self.buffer[..existing]);
        self.buffer.drain(..existing);

        buf = &mut buf[existing..];

        if buf.is_empty() || self.blob.total_len.is_some_and(|v| self.offset >= v) {
            return Ok(existing);
        }

        let chunk = self
            .db
            .lock()
            .unwrap()
            .read_blob_chunk(self.id, self.offset)
            .map_err(io::Error::other)?;

        let Some(chunk) = chunk else {
            return Ok(existing);
        };

        let data = self
            .blob
            .compression
            .decompress(chunk.len as usize, &chunk.data)?;

        self.buffer.extend_from_slice(&data);
        self.offset += chunk.len;

        let extra = buf.len().min(self.buffer.len());
        buf[..extra].copy_from_slice(&self.buffer[..extra]);
        self.buffer.drain(..extra);

        Ok(existing + extra)
    }
}
