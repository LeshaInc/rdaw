mod arrangement;
mod blob;
mod item;
mod source;
mod storage;
mod tempo_map;
mod track;

use slotmap::Key;
pub use uuid::Uuid;

pub use self::arrangement::Arrangement;
pub use self::blob::Blob;
pub use self::item::AudioItem;
pub use self::source::AudioSource;
pub use self::storage::Storage;
pub use self::tempo_map::TempoMap;
pub use self::track::{Track, TrackItems, TrackLinks};

pub trait Object {
    type Id: Key;

    fn uuid(&self) -> Uuid;

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        let _unused = hub;
        callback(self.uuid());
    }
}

#[derive(Debug, Default)]
pub struct Hub {
    pub blobs: Storage<Blob>,
    pub arrangements: Storage<Arrangement>,
    pub tempo_maps: Storage<TempoMap>,
    pub tracks: Storage<Track>,
    pub audio_items: Storage<AudioItem>,
    pub audio_sources: Storage<AudioSource>,
}
