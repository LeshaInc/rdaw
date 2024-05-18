mod audio;

pub use self::audio::AudioItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemId {
    Audio(AudioItemId),
}

impl From<AudioItemId> for ItemId {
    fn from(id: AudioItemId) -> ItemId {
        ItemId::Audio(id)
    }
}
