use std::ops::{Index, IndexMut};

use rdaw_api::{bail, format_err, Error, ErrorKind, Result};
use rdaw_core::collections::{HashMap, HashSet};
use slotmap::SlotMap;

use super::{Object, ObjectId, ObjectKey};

#[derive(Debug)]
pub struct Storage<T: Object> {
    map: SlotMap<T::Id, Entry<T>>,
    dirty_set: HashSet<T::Id>,
    key_to_id: HashMap<ObjectKey, T::Id>,
}

#[derive(Debug)]
struct Entry<T> {
    key: ObjectKey,
    object: Option<T>,
}

impl<T: Object> Storage<T> {
    pub fn new() -> Storage<T> {
        Storage {
            map: SlotMap::default(),
            dirty_set: HashSet::default(),
            key_to_id: HashMap::default(),
        }
    }

    pub fn prepare_insert(&mut self, key: ObjectKey) -> T::Id {
        let id = self.map.insert(Entry { key, object: None });
        self.key_to_id.insert(key, id);
        id
    }

    pub fn finish_insert(&mut self, id: T::Id, object: T) {
        self.map[id].object = Some(object);
        self.dirty_set.insert(id);
    }

    pub fn insert(&mut self, key: ObjectKey, object: T) -> T::Id {
        let id = self.map.insert(Entry {
            key,
            object: Some(object),
        });

        self.dirty_set.insert(id);
        self.key_to_id.insert(key, id);

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

    pub fn get_key(&self, id: T::Id) -> Option<&ObjectKey> {
        self.map.get(id).map(|v| &v.key)
    }

    #[track_caller]
    pub fn get_key_or_err(&self, id: T::Id) -> Result<&ObjectKey> {
        match self.map.get(id).map(|v| &v.key) {
            Some(v) => Ok(v),
            None => Err(err_invalid_id(id)),
        }
    }

    pub fn get_id(&self, key: ObjectKey) -> Option<T::Id> {
        self.key_to_id.get(&key).copied()
    }

    #[track_caller]
    pub fn get_id_or_err(&self, key: ObjectKey) -> Result<T::Id> {
        match self.get_id(key) {
            Some(v) => Ok(v),
            None => Err(err_invalid_key(key)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (T::Id, &ObjectKey, &T)> + '_ {
        self.map
            .iter()
            .flat_map(|(id, entry)| entry.object.as_ref().map(|obj| (id, &entry.key, obj)))
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
        self.dirty_set.clear()
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
fn err_invalid_key(key: ObjectKey) -> Error {
    format_err!(
        ErrorKind::InvalidId,
        "{} doesn't exist in {:?}",
        key.uuid,
        key.document_id
    )
}
