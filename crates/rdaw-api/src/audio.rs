use rdaw_core::time::RealTime;

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub channels: Vec<AudioChannel>,
    pub sample_rate: u32,
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
