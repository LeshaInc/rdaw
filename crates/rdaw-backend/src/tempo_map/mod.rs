use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::time::{BeatTime, Time};
use rdaw_core::time::RealTime;

use crate::{Object, Uuid};

#[derive(Debug, Clone)]
pub struct TempoMap {
    uuid: Uuid,
    beats_per_minute: f32,
    beats_per_bar: u32,
}

impl TempoMap {
    pub fn new(beats_per_minute: f32, beats_per_bar: u32) -> TempoMap {
        TempoMap {
            uuid: Uuid::new_v4(),
            beats_per_minute,
            beats_per_bar,
        }
    }

    pub fn to_real(&self, time: Time) -> RealTime {
        match time {
            Time::Real(t) => t,
            Time::Beat(t) => self.beat_to_real(t),
        }
    }

    pub fn to_beat(&self, time: Time) -> BeatTime {
        match time {
            Time::Real(t) => self.real_to_beat(t),
            Time::Beat(t) => t,
        }
    }

    pub fn real_to_beat(&self, real: RealTime) -> BeatTime {
        let frac_beats = real.as_secs_f64() / 60.0 * f64::from(self.beats_per_minute);
        let subbeat = ((frac_beats - frac_beats.floor()) * (f64::from(u32::MAX) + 1.0)) as u32;
        let whole_beats = frac_beats.floor() as i64;
        let beat = whole_beats.rem_euclid(i64::from(self.beats_per_bar)) as u32;
        let bar = whole_beats.div_euclid(i64::from(self.beats_per_bar)) as i32;
        BeatTime { bar, beat, subbeat }
    }

    pub fn beat_to_real(&self, beat: BeatTime) -> RealTime {
        let whole_beats =
            f64::from(beat.bar) * f64::from(self.beats_per_bar) + f64::from(beat.beat);
        let frac_beats = whole_beats + f64::from(beat.subbeat) / (f64::from(u32::MAX) + 1.0);
        let seconds = frac_beats / f64::from(self.beats_per_minute) * 60.0;
        RealTime::from_secs_f64(seconds)
    }
}

impl Object for TempoMap {
    type Id = TempoMapId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }
}