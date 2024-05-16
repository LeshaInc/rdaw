use std::sync::{Arc, Mutex};
use std::time::Duration;

use rdaw_audio::driver::{Channel, Driver as _, OutStreamDesc};
use rdaw_audio::graph::{CompiledNode, Graph, GraphParams, Inputs, Node, Outputs, Port};
use rdaw_core::sync::spsc::{self, IpcChannel, IpcReceiver, Sender};
use rdaw_pipewire::Driver;

#[derive(Clone, Copy)]
struct Saw {
    outputs: usize,
    freq: f32,
    time: f32,
}

impl Node for Saw {
    fn num_audio_inputs(&self) -> usize {
        0
    }

    fn num_audio_outputs(&self) -> usize {
        self.outputs
    }

    fn compile(&self, _params: &GraphParams) -> Box<dyn CompiledNode> {
        Box::new(*self)
    }
}

impl CompiledNode for Saw {
    fn process(&mut self, params: &GraphParams, _inputs: Inputs<'_>, outputs: Outputs<'_>) {
        for i in 0..params.buffer_size {
            for buffer in outputs.audio.iter_mut() {
                let x = self.time / (params.sample_rate as f32) * self.freq;
                let y = 2.0 * (x - (0.5 + x).floor());
                buffer[i] = y * 0.5;
            }

            self.time += 1.0;
        }
    }
}

#[derive(Clone)]
struct External {
    receiver: Arc<Mutex<IpcReceiver<f32>>>,
}

impl Node for External {
    fn num_audio_inputs(&self) -> usize {
        0
    }

    fn num_audio_outputs(&self) -> usize {
        1
    }

    fn compile(&self, _params: &GraphParams) -> Box<dyn CompiledNode> {
        Box::new(self.clone())
    }
}

impl CompiledNode for External {
    fn process(&mut self, _params: &GraphParams, _inputs: Inputs<'_>, outputs: Outputs<'_>) {
        let mut receiver = self.receiver.lock().unwrap();
        let _ = receiver.recv_slice(outputs.audio[0]);
    }
}

#[derive(Clone)]
struct Sink {
    senders: Vec<Arc<Mutex<Sender<f32>>>>,
}

impl Node for Sink {
    fn num_audio_inputs(&self) -> usize {
        self.senders.len()
    }

    fn num_audio_outputs(&self) -> usize {
        0
    }

    fn compile(&self, _params: &GraphParams) -> Box<dyn CompiledNode> {
        Box::new(self.clone())
    }
}

impl CompiledNode for Sink {
    fn process(&mut self, _params: &GraphParams, inputs: Inputs<'_>, _outputs: Outputs<'_>) {
        for (sender, input) in self.senders.iter_mut().zip(inputs.audio.iter()) {
            let mut sender = sender.lock().unwrap();
            let _ = sender.send_slice(&input.data);
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let sample_rate = 48000;
    let buffer_size = 128;
    let ring_size = buffer_size * 2;

    if let Ok(id) = std::env::var("CHANNEL_ID") {
        let mut sender = unsafe { IpcChannel::<f32>::open(&id) }
            .unwrap()
            .sender()
            .unwrap();

        let mut buf = vec![0.0; buffer_size];
        let mut time = 0.0;

        if let Err(e) = audio_thread_priority::promote_current_thread_to_real_time(
            buffer_size as u32,
            sample_rate,
        ) {
            eprintln!("{e}");
        }

        loop {
            for val in &mut buf {
                let x = time / (sample_rate as f32) * 40.0;
                let y = 2.0 * (x - (0.5 + x).floor());
                *val = y * 0.5;
                time += 1.0;
            }

            if sender.send_slice(&buf).is_err() {
                break;
            }
        }

        return;
    }

    let (left_sender, mut left_receiver) = spsc::channel(ring_size);
    let (right_sender, mut right_receiver) = spsc::channel(ring_size);

    let mut graph = Graph::new(GraphParams {
        sample_rate,
        buffer_size,
    });

    let sine = graph.add_node(Saw {
        outputs: 1,
        freq: 40.0,
        time: 0.0,
    });

    let channel = IpcChannel::create("rdaw", ring_size).unwrap();

    std::process::Command::new(std::env::current_exe().unwrap())
        .env("CHANNEL_ID", channel.id())
        .spawn()
        .unwrap();

    let receiver = channel.receiver().unwrap();

    let external = graph.add_node(External {
        receiver: Arc::new(Mutex::new(receiver)),
    });

    let sink = graph.add_node(Sink {
        senders: vec![
            Arc::new(Mutex::new(left_sender)),
            Arc::new(Mutex::new(right_sender)),
        ],
    });

    graph.connect((sine, Port::Audio(0)), (sink, Port::Audio(0)));
    graph.connect((external, Port::Audio(0)), (sink, Port::Audio(1)));

    let mut compiled_graph = graph.compile();

    std::thread::spawn(move || {
        if let Err(e) = audio_thread_priority::promote_current_thread_to_real_time(
            buffer_size as u32,
            sample_rate,
        ) {
            eprintln!("{e}");
        }

        loop {
            compiled_graph.process();
        }
    });

    let driver = Driver::new().unwrap();

    let mut left_buf = vec![0.0; buffer_size];
    let mut right_buf = vec![0.0; buffer_size];

    let _stream = driver
        .create_out_stream(OutStreamDesc {
            name: "My app".into(),
            sample_rate,
            buffer_size,
            channels: vec![Channel::FL, Channel::FR],
            callback: Box::new(move |data| {
                let _ = left_receiver.try_recv_slice(&mut left_buf);
                let _ = right_receiver.try_recv_slice(&mut right_buf);

                for i in 0..data.samples.len() / 2 {
                    data.samples[2 * i] = left_buf[i];
                    data.samples[2 * i + 1] = right_buf[i];
                }
            }),
        })
        .unwrap();

    std::thread::sleep(Duration::from_secs(5));
}
