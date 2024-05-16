use rdaw_core::time::RealTime;

use crate::BeatMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Time {
    Real(RealTime),
    Beat(BeatTime),
}

impl Time {
    pub fn to_real(self, beat_map: &BeatMap) -> RealTime {
        match self {
            Time::Real(t) => t,
            Time::Beat(t) => t.to_real(beat_map),
        }
    }

    pub fn to_beat(self, beat_map: &BeatMap) -> BeatTime {
        match self {
            Time::Real(t) => BeatTime::from_real(t, beat_map),
            Time::Beat(t) => t,
        }
    }
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

    pub fn from_real(real: RealTime, beat_map: &BeatMap) -> BeatTime {
        let frac_beats = real.as_secs_f64() / 60.0 * f64::from(beat_map.beats_per_minute);
        let subbeat = ((frac_beats - frac_beats.floor()) * (f64::from(u32::MAX) + 1.0)) as u32;
        let whole_beats = frac_beats.floor() as i64;
        let beat = whole_beats.rem_euclid(i64::from(beat_map.beats_per_bar)) as u32;
        let bar = whole_beats.div_euclid(i64::from(beat_map.beats_per_bar)) as i32;
        BeatTime { bar, beat, subbeat }
    }

    pub fn to_real(self, beat_map: &BeatMap) -> RealTime {
        let whole_beats =
            f64::from(self.bar) * f64::from(beat_map.beats_per_bar) + f64::from(self.beat);
        let frac_beats = whole_beats + f64::from(self.subbeat) / (f64::from(u32::MAX) + 1.0);
        let seconds = frac_beats / f64::from(beat_map.beats_per_minute) * 60.0;
        RealTime::from_secs_f64(seconds)
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::*;

    const EPS: RealTime = RealTime::from_nanos(100);

    #[test]
    fn test_conversion() {
        let beat_map = BeatMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        assert_eq!(
            RealTime::from_secs_f64(1.0),
            RealTime::from_nanos(1_000_000_000)
        );

        assert_eq!(
            BeatTime::from_real(RealTime::from_secs_f64(0.0), &beat_map),
            BeatTime::new(0, 0, 0),
        );

        assert_eq!(
            BeatTime::from_real(RealTime::from_secs_f64(60.0), &beat_map),
            BeatTime::new(30, 0, 0),
        );

        assert_eq!(
            BeatTime::new(30, 0, 0).to_real(&beat_map),
            RealTime::from_secs_f64(60.0)
        );

        assert_eq!(
            BeatTime::from_real(RealTime::from_secs_f64(61.25), &beat_map),
            BeatTime::new(30, 2, u32::MAX / 2),
        );

        assert!(BeatTime::new(30, 2, u32::MAX / 2)
            .to_real(&beat_map)
            .approx_eq(RealTime::from_secs_f64(61.25), EPS));
    }

    #[test]
    fn test_random() {
        let mut rng = SmallRng::seed_from_u64(0);

        for _ in 0..1000 {
            let beat_map = BeatMap {
                beats_per_minute: rng.gen_range(50.0..250.0),
                beats_per_bar: rng.gen_range(1..32),
            };

            for _ in 0..1000 {
                let real = RealTime::from_secs_f64(rng.gen_range(0.0..0.5));
                let beat = BeatTime::from_real(real, &beat_map);
                assert!(beat.to_real(&beat_map).approx_eq(real, EPS));
            }
        }
    }
}
