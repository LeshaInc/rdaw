use std::fs::File;
use std::io::Read;

use crate::document::BlobReader;

#[derive(Debug)]
pub struct AssetReader {
    inner: Inner,
}

impl AssetReader {
    pub fn from_file(file: File) -> AssetReader {
        AssetReader {
            inner: Inner::File(file),
        }
    }

    pub fn from_blob(reader: BlobReader) -> AssetReader {
        AssetReader {
            inner: Inner::Blob(reader),
        }
    }
}

#[derive(Debug)]
enum Inner {
    File(File),
    Blob(BlobReader),
}

impl Read for AssetReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            Inner::File(v) => v.read(buf),
            Inner::Blob(v) => v.read(buf),
        }
    }
}
