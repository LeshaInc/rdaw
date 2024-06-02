use std::ops::{Add, Sub};

use fixed::types::I32F32;
use rdaw_core::time::RealTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Time {
    Real(RealTime),
    Beat(BeatTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BeatTime {
    beats: I32F32,
}

impl BeatTime {
    pub const ZERO: BeatTime = BeatTime::new(I32F32::ZERO);
    pub const MIN: BeatTime = BeatTime::new(I32F32::MIN);
    pub const MAX: BeatTime = BeatTime::new(I32F32::MAX);

    pub const fn new(beats: I32F32) -> BeatTime {
        BeatTime { beats }
    }

    pub fn from_beats(beats: i32) -> BeatTime {
        BeatTime::new(I32F32::from_num(beats))
    }

    pub fn from_beats_f32(beats: f32) -> BeatTime {
        BeatTime::new(I32F32::from_num(beats))
    }

    pub fn from_beats_f64(beats: f64) -> BeatTime {
        BeatTime::new(I32F32::from_num(beats))
    }

    pub fn as_beats(self) -> i32 {
        self.beats.to_num()
    }

    pub fn as_beats_f32(self) -> f32 {
        self.beats.to_num()
    }

    pub fn as_beats_f64(self) -> f64 {
        self.beats.to_num()
    }
}

impl Add<BeatTime> for BeatTime {
    type Output = BeatTime;

    fn add(self, rhs: BeatTime) -> Self::Output {
        BeatTime::new(self.beats + rhs.beats)
    }
}

impl Sub<BeatTime> for BeatTime {
    type Output = BeatTime;

    fn sub(self, rhs: BeatTime) -> Self::Output {
        BeatTime::new(self.beats - rhs.beats)
    }
}
