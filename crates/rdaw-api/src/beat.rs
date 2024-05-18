use rdaw_core::time::RealTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Time {
    Real(RealTime),
    Beat(BeatTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BeatTime {
    pub bar: i32,
    pub beat: u32,
    pub subbeat: u32,
}

impl BeatTime {
    pub const ZERO: BeatTime = BeatTime::new(0, 0, 0);
    pub const MIN: BeatTime = BeatTime::new(i32::MIN, 0, 0);
    pub const MAX: BeatTime = BeatTime::new(i32::MAX, u32::MAX, u32::MAX);

    pub const fn new(bar: i32, beat: u32, subbeat: u32) -> BeatTime {
        BeatTime { bar, beat, subbeat }
    }
}
