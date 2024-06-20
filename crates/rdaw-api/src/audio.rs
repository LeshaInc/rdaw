use rdaw_core::time::RealTime;

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioMetadata {
    pub channels: Vec<AudioChannel>,
    pub sample_rate: u32,
    pub sample_format: SampleFormat,
    pub duration: RealTime,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum AudioChannel {
    Unknown,
    Silent,
    Mono,
    FrontLeft,
    FrontRight,
    FrontCenter,
    LowFrequency,
    SideLeft,
    SideRight,
    FrontLeftCenter,
    FrontRightCenter,
    RearCenter,
    RearLeft,
    RearRight,
    TopCenter,
    TopFrontLeft,
    TopFrontCenter,
    TopFrontRight,
    TopRearLeft,
    TopRearCenter,
    TopRearRight,
    RearLeftCenter,
    RearRightCenter,
    FrontLeftWide,
    FrontRightWide,
    LowFrequency2,
    FrontLeftHigh,
    FrontCenterHigh,
    FrontRightHigh,
    TopFrontLeftCenter,
    TopFrontRightCenter,
    TopSideLeft,
    TopSideRight,
    LeftLowFrequency,
    RightLowFrequency,
    BottomCenter,
    BottomLeftCenter,
    BottomRightCenter,
    Aux(u32),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SampleFormat {
    U8,
    I16,
    I32,
    F32,
    F64,
    Other,
}

pub trait AudioInputStream<'media> {
    fn metadata(&self) -> &AudioMetadata;

    fn next_frame(&mut self) -> Result<&[f32]>;
}
