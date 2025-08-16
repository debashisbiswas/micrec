use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let host = cpal::default_host();
    let device = host.default_input_device().unwrap();
    let config = device.default_input_config().unwrap();

    let stream = device
        .build_input_stream(
            &config.into(),
            |data: &[f32], _| {
                process_audio(data);
            },
            |err| eprintln!("Error: {}", err),
            None,
        )
        .unwrap();

    stream.play().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(10));
}

fn process_audio(data: &[f32]) {
    if !data.is_empty() {
        let avg_amplitude = data.iter().map(|&x| x.abs()).sum::<f32>() / data.len() as f32;
        let max_amplitude = data.iter().map(|&x| x.abs()).fold(0.0, f32::max);

        let zero_crossings = data
            .windows(2)
            .filter(|pair| pair[0] * pair[1] < 0.0)
            .count();

        println!(
            "Audio data - Samples: {}, Avg: {:.4}, Max: {:.4}, Zero Crossings: {}",
            data.len(),
            avg_amplitude,
            max_amplitude,
            zero_crossings
        );
    }
}
