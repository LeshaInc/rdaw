mod audio;

pub use self::audio::{AudioItem, AudioItemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemId {
    Audio(AudioItemId),
}

impl From<AudioItemId> for ItemId {
    fn from(id: AudioItemId) -> ItemId {
        ItemId::Audio(id)
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Audio(AudioItem),
}

impl From<AudioItem> for Item {
    fn from(item: AudioItem) -> Item {
        Item::Audio(item)
    }
}
