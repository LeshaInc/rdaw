use rdaw_core::time::RealTime;

use crate::Result;

#[derive(Debug, Clone)]
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
    FL,
    FR,
    FC,
    LFE,
    SL,
    SR,
    FLC,
    FRC,
    RC,
    RL,
    RR,
    TC,
    TFL,
    TFC,
    TFR,
    TRL,
    TRC,
    TRR,
    RLC,
    RRC,
    FLW,
    FRW,
    LFE2,
    FLH,
    FCH,
    FRH,
    TFLC,
    TFRC,
    TSL,
    TSR,
    LLFE,
    RLFE,
    BC,
    BLC,
    BRC,
    Aux(u32),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SampleFormat {
    U8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
    Other,
}

pub trait AudioInputStream<'media> {
    fn metadata(&self) -> AudioMetadata;

    fn next_frame(&mut self, buf: &mut [f32]) -> Result<usize>;
}
