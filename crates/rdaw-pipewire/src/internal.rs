use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;
use std::slice;

use pipewire::channel::{Receiver, Sender};
use pipewire::context::Context;
use pipewire::core::Core;
use pipewire::keys::*;
use pipewire::main_loop::MainLoop;
use pipewire::properties::properties;
use pipewire::registry::{GlobalObject, Registry};
use pipewire::spa::param::audio::{AudioFormat, AudioInfoRaw, MAX_CHANNELS};
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{Object, Pod, Value};
use pipewire::spa::sys::*;
use pipewire::spa::utils::dict::DictRef;
use pipewire::spa::utils::Direction;
use pipewire::stream::{Stream, StreamFlags, StreamListener};
use pipewire::types::ObjectType;
use rdaw_api::audio::AudioChannel;
use rdaw_audio::driver::{OutCallbackData, OutStreamDesc};
use slotmap::SlotMap;

use crate::{Error, Result};

slotmap::new_key_type! {
    pub struct OutStreamId;
}

pub enum Message {
    CreateOutStream {
        sender: oneshot::Sender<Result<OutStreamId>>,
        desc: OutStreamDesc,
    },
    IsOutStreamActive {
        sender: oneshot::Sender<Result<bool>>,
        id: OutStreamId,
    },
    SetOutStreamActive {
        sender: oneshot::Sender<Result<()>>,
        id: OutStreamId,
        active: bool,
    },
    DestroyOutStream {
        id: OutStreamId,
    },
    Terminate,
}

#[derive(Clone)]
pub struct Handle {
    sender: Sender<Message>,
}

impl Handle {
    pub fn new() -> (Handle, Receiver<Message>) {
        let (sender, receiver) = pipewire::channel::channel();
        (Handle { sender }, receiver)
    }

    fn send(&self, message: Message) -> Result<()> {
        self.sender.send(message).map_err(|_| Error::ThreadCrashed)
    }

    fn send_recv<T>(&self, recv: oneshot::Receiver<Result<T>>, message: Message) -> Result<T> {
        let _ = self.sender.send(message);
        recv.recv().map_err(|_| Error::ThreadCrashed)?
    }

    pub fn terminate(&self) -> Result<()> {
        self.send(Message::Terminate)
    }

    pub fn create_out_stream(&self, desc: OutStreamDesc) -> Result<OutStreamId> {
        let (sender, receiver) = oneshot::channel();
        self.send_recv(receiver, Message::CreateOutStream { sender, desc })
    }

    pub fn is_out_stream_active(&self, id: OutStreamId) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();
        self.send_recv(receiver, Message::IsOutStreamActive { sender, id })
    }

    pub fn set_out_stream_active(&self, id: OutStreamId, active: bool) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.send_recv(receiver, Message::SetOutStreamActive { sender, id, active })
    }

    pub fn destroy_out_stream(&self, id: OutStreamId) -> Result<()> {
        self.send(Message::DestroyOutStream { id })
    }
}

pub struct PwThread {
    main_loop: MainLoop,
    core: Core,
    registry: Registry,

    out_streams: RefCell<SlotMap<OutStreamId, OutStream>>,
}

struct OutStream {
    active: bool,
    stream: Stream,
    _listener: StreamListener<()>,
}

impl PwThread {
    pub fn new() -> Result<PwThread> {
        let main_loop = MainLoop::new(None)?;
        let context = Context::new(&main_loop)?;
        let core = context.connect(None)?;
        let registry = core.get_registry()?;

        Ok(PwThread {
            main_loop,
            core,
            registry,
            out_streams: Default::default(),
        })
    }

    pub fn run(self, receiver: Receiver<Message>) {
        let self_rc = Rc::new(self);

        let clone = self_rc.clone();
        let clone1 = self_rc.clone();
        let _listener = self_rc
            .registry
            .add_listener_local()
            .global(move |obj| clone.on_object_added(obj))
            .global_remove(move |obj_id| clone1.on_object_removed(obj_id))
            .register();

        let main_loop = self_rc.main_loop.clone();
        let _receiver = receiver.attach(main_loop.loop_(), move |msg| self_rc.handle_message(msg));

        main_loop.run();
    }

    fn on_object_added(&self, obj: &GlobalObject<&DictRef>) {
        if obj.type_ == ObjectType::Node {
            let Some(props) = &obj.props else { return };
            let Some(class) = props.get(&MEDIA_CLASS) else {
                return;
            };

            if class == "Audio/Sink" {
                // dbg!(props.get(&NODE_DESCRIPTION));
                // dbg!(props.get(&NODE_NAME));
                // dbg!(props.get(&NODE_NICK));
            }

            // dbg!(obj);
        }
        // TODO
    }

    fn on_object_removed(&self, _obj_id: u32) {
        // TODO
    }

    fn handle_message(&self, message: Message) {
        match message {
            Message::CreateOutStream { sender, desc } => {
                let _ = sender.send(self.create_out_stream(desc));
            }
            Message::IsOutStreamActive { sender, id } => {
                let _ = sender.send(self.is_out_stream_active(id));
            }
            Message::SetOutStreamActive { sender, id, active } => {
                let _ = sender.send(self.set_out_stream_active(id, active));
            }
            Message::DestroyOutStream { id } => self.destroy_out_stream(id),
            Message::Terminate => self.terminate(),
        }
    }

    fn create_out_stream(&self, desc: OutStreamDesc) -> Result<OutStreamId> {
        let OutStreamDesc {
            name,
            sample_rate,
            channels,
            mut callback,
            buffer_size,
        } = desc;

        let num_channels = channels.len();

        let props = properties! {
            *MEDIA_TYPE => "Audio",
            *MEDIA_ROLE => "Production",
            *MEDIA_CATEGORY => "Playback",
            *AUDIO_CHANNELS => num_channels.to_string().as_bytes(),
            *NODE_LATENCY => format!("{buffer_size}/{sample_rate}").as_bytes(),
        };

        let stream = Stream::new(&self.core, &name, props)?;

        let listener = stream
            .add_local_listener::<()>()
            .process(move |stream, _| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };

                for data in buffer.datas_mut() {
                    let Some(samples) = data.data() else {
                        continue;
                    };

                    let samples = &mut transmute_out_buffer(samples);
                    let len = samples.len().min(buffer_size * num_channels);
                    let samples = &mut samples[..len];

                    let num_frames = samples.len() / num_channels;
                    let chunk_size = std::mem::size_of_val(samples) as u32;
                    let chunk_stride = (num_channels * size_of::<f32>()) as i32;

                    (callback)(OutCallbackData {
                        samples,
                        num_channels,
                        num_frames,
                    });

                    let chunk = data.chunk_mut();
                    *chunk.offset_mut() = 0;
                    *chunk.size_mut() = chunk_size;
                    *chunk.stride_mut() = chunk_stride;
                }
            })
            .register()?;

        let audio_info = serialize_audio_info(sample_rate, &channels)?;
        let mut params = [Pod::from_bytes(&audio_info).unwrap()];

        stream.connect(
            Direction::Output,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut params,
        )?;

        let out_stream = OutStream {
            active: true,
            stream,
            _listener: listener,
        };

        let id = self.out_streams.borrow_mut().insert(out_stream);

        Ok(id)
    }

    fn is_out_stream_active(&self, id: OutStreamId) -> Result<bool> {
        let out_streams = self.out_streams.borrow();
        let stream = out_streams.get(id).ok_or_else(|| Error::InvalidStreamId)?;
        Ok(stream.active)
    }

    fn set_out_stream_active(&self, id: OutStreamId, active: bool) -> Result<()> {
        let mut out_streams = self.out_streams.borrow_mut();
        let stream = out_streams
            .get_mut(id)
            .ok_or_else(|| Error::InvalidStreamId)?;

        stream.stream.set_active(active)?;
        stream.active = active;

        Ok(())
    }

    fn destroy_out_stream(&self, id: OutStreamId) {
        self.out_streams.borrow_mut().remove(id);
    }

    fn terminate(&self) {
        self.main_loop.quit();
    }
}

fn serialize_audio_info(sample_rate: u32, channels: &[AudioChannel]) -> Result<Vec<u8>> {
    if channels.len() > MAX_CHANNELS {
        return Err(Error::TooManyChannels);
    }

    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    audio_info.set_rate(sample_rate);
    audio_info.set_channels(channels.len() as u32);

    let mut position = [0; MAX_CHANNELS];

    for (i, &channel) in channels.iter().enumerate() {
        position[i] = match channel {
            AudioChannel::Silent => SPA_AUDIO_CHANNEL_NA,
            AudioChannel::Mono => SPA_AUDIO_CHANNEL_MONO,
            AudioChannel::FrontLeft => SPA_AUDIO_CHANNEL_FL,
            AudioChannel::FrontRight => SPA_AUDIO_CHANNEL_FR,
            AudioChannel::FrontCenter => SPA_AUDIO_CHANNEL_FC,
            AudioChannel::LowFrequency => SPA_AUDIO_CHANNEL_LFE,
            AudioChannel::SideLeft => SPA_AUDIO_CHANNEL_SL,
            AudioChannel::SideRight => SPA_AUDIO_CHANNEL_SR,
            AudioChannel::FrontLeftCenter => SPA_AUDIO_CHANNEL_FLC,
            AudioChannel::FrontRightCenter => SPA_AUDIO_CHANNEL_FRC,
            AudioChannel::RearCenter => SPA_AUDIO_CHANNEL_RC,
            AudioChannel::RearLeft => SPA_AUDIO_CHANNEL_RL,
            AudioChannel::RearRight => SPA_AUDIO_CHANNEL_RR,
            AudioChannel::TopCenter => SPA_AUDIO_CHANNEL_TC,
            AudioChannel::TopFrontLeft => SPA_AUDIO_CHANNEL_TFL,
            AudioChannel::TopFrontCenter => SPA_AUDIO_CHANNEL_TFC,
            AudioChannel::TopFrontRight => SPA_AUDIO_CHANNEL_TFR,
            AudioChannel::TopRearLeft => SPA_AUDIO_CHANNEL_TRL,
            AudioChannel::TopRearCenter => SPA_AUDIO_CHANNEL_TRC,
            AudioChannel::TopRearRight => SPA_AUDIO_CHANNEL_TRR,
            AudioChannel::RearLeftCenter => SPA_AUDIO_CHANNEL_RLC,
            AudioChannel::RearRightCenter => SPA_AUDIO_CHANNEL_RRC,
            AudioChannel::FrontLeftWide => SPA_AUDIO_CHANNEL_FLW,
            AudioChannel::FrontRightWide => SPA_AUDIO_CHANNEL_FRW,
            AudioChannel::LowFrequency2 => SPA_AUDIO_CHANNEL_LFE2,
            AudioChannel::FrontLeftHigh => SPA_AUDIO_CHANNEL_FLH,
            AudioChannel::FrontCenterHigh => SPA_AUDIO_CHANNEL_FCH,
            AudioChannel::FrontRightHigh => SPA_AUDIO_CHANNEL_FRH,
            AudioChannel::TopFrontLeftCenter => SPA_AUDIO_CHANNEL_TFLC,
            AudioChannel::TopFrontRightCenter => SPA_AUDIO_CHANNEL_TFRC,
            AudioChannel::TopSideLeft => SPA_AUDIO_CHANNEL_TSL,
            AudioChannel::TopSideRight => SPA_AUDIO_CHANNEL_TSR,
            AudioChannel::LeftLowFrequency => SPA_AUDIO_CHANNEL_LLFE,
            AudioChannel::RightLowFrequency => SPA_AUDIO_CHANNEL_RLFE,
            AudioChannel::BottomCenter => SPA_AUDIO_CHANNEL_BC,
            AudioChannel::BottomLeftCenter => SPA_AUDIO_CHANNEL_BLC,
            AudioChannel::BottomRightCenter => SPA_AUDIO_CHANNEL_BRC,
            AudioChannel::Aux(idx) => match SPA_AUDIO_CHANNEL_START_Aux.checked_add(idx) {
                Some(v) if v < SPA_AUDIO_CHANNEL_LAST_Aux => v,
                _ => SPA_AUDIO_CHANNEL_UNKNOWN,
            },
            _ => SPA_AUDIO_CHANNEL_UNKNOWN,
        }
    }

    audio_info.set_position(position);

    let values = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(Object {
            type_: SPA_TYPE_OBJECT_Format,
            id: SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )?;

    Ok(values.0.into_inner())
}

fn transmute_out_buffer(data: &mut [u8]) -> &mut [f32] {
    assert!(data.len() % size_of::<f32>() == 0);
    let len = data.len() / size_of::<f32>();
    let ptr = data.as_mut_ptr() as *mut f32;
    assert!(ptr.is_aligned());
    unsafe { slice::from_raw_parts_mut(ptr, len) }
}
