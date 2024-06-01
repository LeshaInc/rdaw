use std::ops::{Index, IndexMut};

use slotmap::SlotMap;

use crate::arrangement::Arrangement;
use crate::blob::Blob;
use crate::item::AudioItem;
use crate::source::AudioSource;
use crate::tempo_map::TempoMap;
use crate::track::Track;
use crate::Object;

#[derive(Debug, Default)]
pub struct Hub {
    pub arrangements: Storage<Arrangement>,
    pub audio_items: Storage<AudioItem>,
    pub audio_sources: Storage<AudioSource>,
    pub blobs: Storage<Blob>,
    pub tempo_maps: Storage<TempoMap>,
    pub tracks: Storage<Track>,
}

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

    pub fn contains_id(&self, id: T::Id) -> bool {
        self.map.contains_key(id)
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

    pub fn get_disjoint_mut<const N: usize>(&mut self, ids: [T::Id; N]) -> Option<[&mut T; N]> {
        self.map
            .get_disjoint_mut(ids)
            .map(|arr| arr.map(|v| &mut v.object))
    }

    pub fn iter(&self) -> impl Iterator<Item = (T::Id, &T)> + '_ {
        self.map.iter().map(|(id, entry)| (id, &entry.object))
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
