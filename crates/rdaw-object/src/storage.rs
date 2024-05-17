use std::ops::{Index, IndexMut};

use slotmap::SlotMap;

use crate::Object;

#[derive(Debug)]
pub struct Storage<T: Object> {
    map: SlotMap<T::Id, Entry<T>>,
}

#[derive(Debug)]
struct Entry<T> {
    object: T,
}

impl<T: Object> Storage<T> {
    pub fn new() -> Storage<T> {
        Storage {
            map: SlotMap::default(),
        }
    }

    pub fn insert(&mut self, object: T) -> T::Id {
        self.map.insert(Entry { object })
    }

    pub fn get(&self, id: T::Id) -> Option<&T> {
        self.map.get(id).map(|v| &v.object)
    }

    pub fn get_mut(&mut self, id: T::Id) -> Option<&mut T> {
        self.map.get_mut(id).map(|v| &mut v.object)
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
