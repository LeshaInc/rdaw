use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::time::{BeatTime, Time};
use rdaw_core::time::RealTime;

use crate::document;
use crate::object::{DeserializationContext, Object, SerializationContext};

#[derive(Debug, Clone)]
pub struct TempoMap {
    beats_per_minute: f32,
}

impl TempoMap {
    pub fn new(beats_per_minute: f32) -> TempoMap {
        TempoMap { beats_per_minute }
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
        let beats = real.as_secs_f64() / 60.0 * f64::from(self.beats_per_minute);
        BeatTime::from_beats_f64(beats)
    }

    pub fn beat_to_real(&self, beat: BeatTime) -> RealTime {
        let beats = beat.as_beats_f64();
        let seconds = beats / f64::from(self.beats_per_minute) * 60.0;
        RealTime::from_secs_f64(seconds)
    }
}

impl Object for TempoMap {
    type Id = TempoMapId;

    fn serialize(&self, _ctx: &SerializationContext<'_>) -> Result<Vec<u8>, document::Error> {
        todo!()
    }

    fn deserialize(_ctx: &DeserializationContext<'_>, _data: &[u8]) -> Result<Self, document::Error>
    where
        Self: Sized,
    {
        todo!()
    }
}
