mod audio;

use serde::{Deserialize, Serialize};

pub use self::audio::AudioItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ItemKind {
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemId {
    Audio(AudioItemId),
}

impl From<AudioItemId> for ItemId {
    fn from(id: AudioItemId) -> ItemId {
        ItemId::Audio(id)
    }
}
