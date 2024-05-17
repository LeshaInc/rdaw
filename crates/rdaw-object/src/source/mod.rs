mod audio;

pub use self::audio::{AudioSource, AudioSourceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceId {
    Audio(AudioSourceId),
}

#[derive(Debug, Clone)]
pub enum Source {
    Audio(AudioSource),
}

impl From<AudioSourceId> for SourceId {
    fn from(id: AudioSourceId) -> SourceId {
        SourceId::Audio(id)
    }
}

impl From<AudioSource> for Source {
    fn from(item: AudioSource) -> Source {
        Source::Audio(item)
    }
}
