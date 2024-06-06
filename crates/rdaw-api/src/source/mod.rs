mod audio;

pub use self::audio::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceId {
    Audio(AudioSourceId),
}

impl From<AudioSourceId> for SourceId {
    fn from(id: AudioSourceId) -> SourceId {
        SourceId::Audio(id)
    }
}
