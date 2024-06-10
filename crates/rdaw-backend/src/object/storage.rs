use std::ops::{Index, IndexMut};

use slotmap::SlotMap;

use super::{Metadata, Object};

#[derive(Debug)]
pub struct Storage<T: Object> {
    map: SlotMap<T::Id, Entry<T>>,
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
        }
    }

    pub fn prepare_insert(&mut self, metadata: Metadata) -> T::Id {
        self.map.insert(Entry {
            metadata,
            object: None,
        })
    }

    pub fn finish_insert(&mut self, id: T::Id, object: T) {
        self.map[id].object = Some(object);
    }

    pub fn insert(&mut self, metadata: Metadata, object: T) -> T::Id {
        self.map.insert(Entry {
            metadata,
            object: Some(object),
        })
    }

    pub fn has(&self, id: T::Id) -> bool {
        self.map.get(id).is_some_and(|v| v.object.is_some())
    }

    pub fn get(&self, id: T::Id) -> Option<&T> {
        self.map.get(id).and_then(|v| v.object.as_ref())
    }

    pub fn get_mut(&mut self, id: T::Id) -> Option<&mut T> {
        self.map.get_mut(id).and_then(|v| v.object.as_mut())
    }

    pub fn get_disjoint_mut<const N: usize>(&mut self, ids: [T::Id; N]) -> Option<[&mut T; N]> {
        self.map.get_disjoint_mut(ids).and_then(|arr| {
            if arr.iter().any(|v| v.object.is_none()) {
                return None;
            }
            Some(arr.map(|v| v.object.as_mut().unwrap()))
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (T::Id, &Metadata, &T)> + '_ {
        self.map
            .iter()
            .flat_map(|(id, entry)| entry.object.as_ref().map(|obj| (id, &entry.metadata, obj)))
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