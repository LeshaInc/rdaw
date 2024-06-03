use rdaw_api::audio::AudioChannel;

pub trait Driver: Send + Sync + 'static {
    type Error: Send + Sync + 'static;
    type OutStream: OutStream;

    fn create_out_stream(&self, desc: OutStreamDesc) -> Result<Self::OutStream, Self::Error>;
}

pub struct OutStreamDesc {
    pub name: String,
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub channels: Vec<AudioChannel>,
    pub callback: Box<dyn FnMut(OutCallbackData<'_>) + Send + 'static>,
}

pub struct OutCallbackData<'a> {
    pub num_channels: usize,
    pub num_frames: usize,
    pub samples: &'a mut [f32],
}

pub trait OutStream: Send + Sync + 'static {
    type Error: Send + Sync + 'static;

    fn is_active(&self) -> Result<bool, Self::Error>;

    fn set_active(&self, active: bool) -> Result<(), Self::Error>;
}
