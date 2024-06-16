use std::ops::{Index, IndexMut};

use rdaw_api::document::DocumentId;
use rdaw_api::{format_err, Error, ErrorKind, Result};
use slotmap::SlotMap;

use super::Document;

#[derive(Debug, Default)]
pub struct DocumentStorage {
    map: SlotMap<DocumentId, Document>,
}

impl DocumentStorage {
    pub fn insert(&mut self, document: Document) -> DocumentId {
        self.map.insert(document)
    }

    pub fn has(&self, id: DocumentId) -> bool {
        self.map.contains_key(id)
    }

    #[track_caller]
    pub fn ensure_has(&self, id: DocumentId) -> Result<()> {
        if self.has(id) {
            Ok(())
        } else {
            Err(err_invalid_id(id))
        }
    }

    pub fn get(&self, id: DocumentId) -> Option<&Document> {
        self.map.get(id)
    }

    #[track_caller]
    pub fn get_or_err(&self, id: DocumentId) -> Result<&Document> {
        match self.get(id) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }

    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.map.get_mut(id)
    }

    #[track_caller]
    pub fn get_mut_or_err(&mut self, id: DocumentId) -> Result<&mut Document> {
        match self.get_mut(id) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }
}

impl Index<DocumentId> for DocumentStorage {
    type Output = Document;

    fn index(&self, index: DocumentId) -> &Document {
        self.get(index).unwrap()
    }
}

impl IndexMut<DocumentId> for DocumentStorage {
    fn index_mut(&mut self, index: DocumentId) -> &mut Document {
        self.get_mut(index).unwrap()
    }
}

#[track_caller]
fn err_invalid_id(id: DocumentId) -> Error {
    format_err!(ErrorKind::InvalidId, "{id:?} doesn't exist")
}
