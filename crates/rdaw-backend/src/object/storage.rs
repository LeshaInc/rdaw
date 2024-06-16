use std::ops::{Index, IndexMut};

use rdaw_api::{bail, format_err, Error, ErrorKind, Result};
use rdaw_core::collections::{HashMap, HashSet};
use rdaw_core::Uuid;
use slotmap::SlotMap;

use super::{Metadata, Object, ObjectId};

#[derive(Debug)]
pub struct Storage<T: Object> {
    map: SlotMap<T::Id, Entry<T>>,
    dirty_set: HashSet<T::Id>,
    uuid_to_id: HashMap<Uuid, T::Id>,
}

#[derive(Debug)]
struct Entry<T> {
    metadata: Metadata,
    object: Option<T>,
}

impl<T: Object> Storage<T> {
    pub fn new() -> Storage<T> {
        Storage {
            map: SlotMap::default(),
            dirty_set: HashSet::default(),
            uuid_to_id: HashMap::default(),
        }
    }

    pub fn prepare_insert(&mut self, metadata: Metadata) -> T::Id {
        let id = self.map.insert(Entry {
            metadata,
            object: None,
        });

        self.uuid_to_id.insert(metadata.uuid, id);

        id
    }

    pub fn finish_insert(&mut self, id: T::Id, object: T) {
        self.map[id].object = Some(object);
        self.dirty_set.insert(id);
    }

    pub fn insert(&mut self, metadata: Metadata, object: T) -> T::Id {
        let id = self.map.insert(Entry {
            metadata,
            object: Some(object),
        });

        self.dirty_set.insert(id);
        self.uuid_to_id.insert(metadata.uuid, id);

        id
    }

    pub fn has(&self, id: T::Id) -> bool {
        self.map.get(id).is_some_and(|v| v.object.is_some())
    }

    #[track_caller]
    pub fn ensure_has(&self, id: T::Id) -> Result<()> {
        if self.has(id) {
            Ok(())
        } else {
            Err(err_invalid_id(id))
        }
    }

    pub fn get(&self, id: T::Id) -> Option<&T> {
        self.map.get(id).and_then(|v| v.object.as_ref())
    }

    #[track_caller]
    pub fn get_or_err(&self, id: T::Id) -> Result<&T> {
        match self.get(id) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }

    pub fn get_mut(&mut self, id: T::Id) -> Option<&mut T> {
        self.map.get_mut(id).and_then(|v| v.object.as_mut())
    }

    #[track_caller]
    pub fn get_mut_or_err(&mut self, id: T::Id) -> Result<&mut T> {
        match self.get_mut(id) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }

    pub fn get_disjoint_mut<const N: usize>(&mut self, ids: [T::Id; N]) -> Option<[&mut T; N]> {
        self.map.get_disjoint_mut(ids).and_then(|arr| {
            if arr.iter().any(|v| v.object.is_none()) {
                return None;
            }
            Some(arr.map(|v| v.object.as_mut().unwrap()))
        })
    }

    #[track_caller]
    pub fn get_disjoint_mut_or_err<const N: usize>(
        &mut self,
        ids: [T::Id; N],
    ) -> Result<[&mut T; N]> {
        for id in ids {
            self.ensure_has(id)?;
        }

        let Some(arr) = self.map.get_disjoint_mut(ids) else {
            bail!(ErrorKind::Other, "duplicate ids in get_disjoint_mut");
        };

        Ok(arr.map(|v| v.object.as_mut().unwrap()))
    }

    pub fn get_metadata(&self, id: T::Id) -> Option<&Metadata> {
        self.map.get(id).map(|v| &v.metadata)
    }

    #[track_caller]
    pub fn get_metadata_or_err(&self, id: T::Id) -> Result<&Metadata> {
        match self.map.get(id).map(|v| &v.metadata) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }

    pub fn lookup_uuid(&self, uuid: Uuid) -> Option<T::Id> {
        self.uuid_to_id.get(&uuid).copied()
    }

    #[track_caller]
    pub fn lookup_uuid_or_err(&self, uuid: Uuid) -> Result<T::Id> {
        match self.lookup_uuid(uuid) {
            Some(v) => Ok(v),
            None => Err(err_invalid_uuid(uuid)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (T::Id, &Metadata, &T)> + '_ {
        self.map
            .iter()
            .flat_map(|(id, entry)| entry.object.as_ref().map(|obj| (id, &entry.metadata, obj)))
    }

    pub fn mark_dirty(&mut self, id: T::Id) {
        if self.has(id) {
            self.dirty_set.insert(id);
        }
    }

    pub fn is_dirty(&self, id: T::Id) -> bool {
        self.dirty_set.contains(&id)
    }

    pub fn clear_all_dirty(&mut self) {
        self.dirty_set.clear();
    }
}

impl<T: Object> Index<T::Id> for Storage<T> {
    type Output = T;

    fn index(&self, index: T::Id) -> &T {
        self.get(index).unwrap()
    }
}

impl<T: Object> IndexMut<T::Id> for Storage<T> {
    fn index_mut(&mut self, index: T::Id) -> &mut T {
        self.get_mut(index).unwrap()
    }
}

impl<T: Object> Default for Storage<T> {
    fn default() -> Storage<T> {
        Storage::new()
    }
}

#[track_caller]
fn err_invalid_id<I: ObjectId>(id: I) -> Error {
    format_err!(ErrorKind::InvalidId, "{id:?} doesn't exist")
}

#[track_caller]
fn err_invalid_uuid(uuid: Uuid) -> Error {
    format_err!(ErrorKind::InvalidId, "{uuid} doesn't exist")
}
