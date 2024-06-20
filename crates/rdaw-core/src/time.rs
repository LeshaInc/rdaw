use std::ops::{Add, Sub};

use serde::{Deserialize, Serialize};

const NANOS_IN_SEC: i64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RealTime {
    nanos: i64,
}

impl RealTime {
    pub const ZERO: RealTime = RealTime::from_nanos(0);
    pub const MIN: RealTime = RealTime::from_nanos(i64::MIN);
    pub const MAX: RealTime = RealTime::from_nanos(i64::MAX);

    pub const fn from_nanos(nanos: i64) -> RealTime {
        RealTime { nanos }
    }

    pub fn from_secs(secs: i64) -> RealTime {
        RealTime::from_nanos(secs * NANOS_IN_SEC)
    }

    pub fn from_secs_f32(secs: f32) -> RealTime {
        RealTime::from_nanos((secs * (NANOS_IN_SEC as f32)) as i64)
    }

    pub fn from_secs_f64(secs: f64) -> RealTime {
        RealTime::from_nanos((secs * (NANOS_IN_SEC as f64)) as i64)
    }

    pub fn as_nanos(self) -> i64 {
        self.nanos
    }

    pub fn as_secs(self) -> i64 {
        self.nanos / 1_000_000_000
    }

    pub fn as_secs_f32(self) -> f32 {
        (self.nanos as f32) / (NANOS_IN_SEC as f32)
    }

    pub fn as_secs_f64(self) -> f64 {
        (self.nanos as f64) / (NANOS_IN_SEC as f64)
    }

    pub fn approx_eq(self, other: RealTime, eps: RealTime) -> bool {
        let diff = if self.nanos > other.nanos {
            self.nanos - other.nanos
        } else {
            other.nanos - self.nanos
        };

        diff <= eps.nanos.abs()
    }
}

impl Add<RealTime> for RealTime {
    type Output = RealTime;

    fn add(self, rhs: RealTime) -> RealTime {
        RealTime {
            nanos: self.nanos + rhs.nanos,
        }
    }
}

impl Sub<RealTime> for RealTime {
    type Output = RealTime;

    fn sub(self, rhs: RealTime) -> RealTime {
        RealTime {
            nanos: self.nanos - rhs.nanos,
        }
    }
}
