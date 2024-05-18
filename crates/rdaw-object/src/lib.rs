mod beat_map;
mod blob;
mod item;
mod source;
mod storage;
mod track;

use slotmap::Key;
pub use uuid::Uuid;

pub use self::beat_map::BeatMap;
pub use self::blob::Blob;
pub use self::item::AudioItem;
pub use self::source::AudioSource;
pub use self::storage::Storage;
pub use self::track::Track;

pub trait Object {
    type Id: Key;

    fn uuid(&self) -> Uuid;

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        let _ = (hub, callback);
    }
}

#[derive(Debug, Default)]
pub struct Hub {
    pub blobs: Storage<Blob>,
    pub tracks: Storage<Track>,
    pub audio_items: Storage<AudioItem>,
    pub audio_sources: Storage<AudioSource>,
}
