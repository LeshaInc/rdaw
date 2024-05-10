use std::fmt;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilentHint {
    Unspecified,
    Silent,
    NotSilent,
}

#[derive(Clone)]
pub struct AudioBuffer {
    pub silent_hint: SilentHint,
    pub data: Box<[f32]>,
}

impl AudioBuffer {
    pub fn new(size: usize) -> AudioBuffer {
        AudioBuffer {
            silent_hint: SilentHint::Silent,
            data: vec![0.0; size].into(),
        }
    }

    pub fn clear(&mut self) {
        self.silent_hint = SilentHint::Silent;
        self.data.fill(0.0);
    }
}

impl fmt::Debug for AudioBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioBuffer")
            .field("silent_hint", &self.silent_hint)
            .field("len", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl Deref for AudioBuffer {
    type Target = [f32];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for AudioBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
