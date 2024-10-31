use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp::{
    interpolate::linear::Linear,
    signal::{Signal},
};
use std::sync::Arc;
use std::sync::Mutex;

const QUACK_FILENAME: &str = "assets/quack.mp3";

fn main() {
    match playback() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("Error during playback:\n {}", e);
            std::process::exit(1)
        }
    }
}

fn playback() -> Result<(), Box<dyn std::error::Error>> {
    let mp3_data = std::fs::read(QUACK_FILENAME)?;

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");

    let supported_configs_range = device.supported_output_configs()
        .expect("error while querying device configs");
    
    let out_config = supported_configs_range
        .filter(|config| config.channels() == 2)
        .max_by(|a, b| a.max_sample_rate().cmp(&b.max_sample_rate()))
        .expect("no supported config")
        .with_max_sample_rate();

    let (header, samples) = puremp3::read_mp3(&mp3_data[..])?;

    let source = dasp::signal::from_iter(samples.map(|sample| [sample.0 as f32, sample.1 as f32]));
    let frames: Vec<[f32; 2]> = source
        .from_hz_to_hz(
            Linear::new([0.0, 0.0], [0.0, 0.0]),
            header.sample_rate.hz() as f64,
            out_config.sample_rate().0 as f64
        )
        .until_exhausted()
        .collect();

    println!("Converted {} frames", frames.len());

    let samples = Arc::new(Mutex::new(frames.clone().into_iter()));

    let err_fn = |err| eprintln!("An error occurred on stream: {}", err);

    let stream = device.build_output_stream(
        &out_config.config(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut samples = samples.lock().unwrap();
            for chunk in data.chunks_mut(2) {
                if let Some(frame) = samples.next() {
                    chunk[0] = frame[0];
                    chunk[1] = frame[1];
                } else {
                    chunk[0] = 0.0;
                    chunk[1] = 0.0;
                }
            }
        },
        err_fn,
        None
    )?;

    stream.play()?;

    let duration = std::time::Duration::from_secs_f64(frames.len() as f64 / out_config.sample_rate().0 as f64);

    std::thread::sleep(duration);

    Ok(())
}