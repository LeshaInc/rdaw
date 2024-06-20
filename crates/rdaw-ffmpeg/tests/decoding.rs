use std::fs::File;
use std::path::PathBuf;

use rdaw_api::audio::{AudioChannel, AudioMetadata, SampleFormat};
use rdaw_api::media::{MediaInput as _, OpenMediaInput as _};
use rdaw_api::Result;
use rdaw_core::time::RealTime;
use rdaw_ffmpeg::MediaInput;

#[test]
fn decode_ogg() -> Result<()> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/samples/220_Hz_sine_wave.ogg");

    let mut media = MediaInput::open(File::open(path)?)?;
    let mut stream = media.get_audio_stream()?.unwrap();

    assert_eq!(
        stream.metadata(),
        &AudioMetadata {
            channels: vec![AudioChannel::FrontCenter],
            sample_rate: 44100,
            sample_format: SampleFormat::F32,
            duration: RealTime::from_secs(5)
        }
    );

    let mut samples = vec![];

    loop {
        let frame = stream.next_frame()?;
        if frame.is_empty() {
            break;
        }

        samples.extend_from_slice(frame);
    }

    assert_eq!(samples.len(), 44100 * 5);

    Ok(())
}
