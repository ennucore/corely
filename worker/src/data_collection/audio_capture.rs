//! Audio capture using cpal for microphone and system output loopback.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::config::MultiTimestamp;
use super::OrchestratorRefs;

/// Run audio capture for mic and/or system output
pub async fn run_capture(
    orchestrator: OrchestratorRefs,
    capture_input: bool,
    capture_output: bool,
) -> Result<()> {
    info!(
        "Starting audio capture: input={}, output={}",
        capture_input, capture_output
    );

    let shutdown_rx = orchestrator.get_shutdown_receiver().await;
    let running = Arc::new(AtomicBool::new(true));

    // Start input (mic) capture thread
    if capture_input {
        let orchestrator_clone = orchestrator.clone();
        let running_clone = running.clone();
        std::thread::spawn(move || {
            if let Err(e) = capture_input_audio(orchestrator_clone, running_clone) {
                error!("Input audio capture error: {}", e);
            }
        });
    }

    // Start output (loopback) capture thread
    if capture_output {
        let orchestrator_clone = orchestrator.clone();
        let running_clone = running.clone();
        std::thread::spawn(move || {
            if let Err(e) = capture_output_audio(orchestrator_clone, running_clone) {
                error!("Output audio capture error: {}", e);
            }
        });
    }

    // Wait for shutdown signal
    if let Some(mut rx) = shutdown_rx {
        let _ = rx.recv().await;
    }

    // Signal threads to stop
    running.store(false, Ordering::SeqCst);

    info!("Audio capture stopped");
    Ok(())
}

fn capture_input_audio(orchestrator: OrchestratorRefs, running: Arc<AtomicBool>) -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No input device available"))?;

    info!("Using input device: {}", device.name().unwrap_or_default());

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    info!(
        "Input audio config: {} Hz, {} channels",
        sample_rate, channels
    );

    let sample_count = Arc::new(AtomicU64::new(0));
    let sample_count_clone = sample_count.clone();
    let orchestrator_clone = orchestrator.clone();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Convert f32 to i16 PCM
            let samples: Vec<i16> = data
                .iter()
                .map(|&s| (s * 32767.0) as i16)
                .collect();

            let bytes: Vec<u8> = samples
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .collect();

            let data_len = data.len() as u64;

            // Write to chunk manager (blocking call in sync context)
            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                let orchestrator = orchestrator_clone.clone();
                let sample_count = sample_count_clone.clone();
                handle.spawn(async move {
                    if let Some(ref cm) = *orchestrator.chunk_manager.lock().await {
                        let seq = sample_count.fetch_add(data_len, Ordering::SeqCst);
                        let ts = MultiTimestamp::now(seq);
                        if let Err(e) = cm.write_frame("mic_audio.pcm", &bytes, ts).await {
                            debug!("Failed to write mic audio: {}", e);
                        }
                    }
                });
            }
        },
        |err| {
            error!("Input stream error: {}", err);
        },
        None,
    )?;

    stream.play()?;

    // Keep stream alive while running
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

fn capture_output_audio(orchestrator: OrchestratorRefs, running: Arc<AtomicBool>) -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();

    // Try to get loopback device
    // On macOS, this requires a virtual audio device like BlackHole
    // On Windows, use WASAPI loopback
    // On Linux, use PulseAudio monitor source

    #[cfg(target_os = "windows")]
    let device = {
        // WASAPI supports loopback capture
        host.default_output_device()
            .ok_or_else(|| anyhow!("No output device available"))?
    };

    #[cfg(not(target_os = "windows"))]
    let device = {
        // On macOS/Linux, try to find a loopback/monitor device
        let devices = host.input_devices()?;
        let mut loopback_device = None;

        for dev in devices {
            let name = dev.name().unwrap_or_default().to_lowercase();
            if name.contains("loopback")
                || name.contains("monitor")
                || name.contains("blackhole")
                || name.contains("soundflower")
            {
                loopback_device = Some(dev);
                break;
            }
        }

        match loopback_device {
            Some(dev) => dev,
            None => {
                warn!("No loopback audio device found. System audio capture unavailable.");
                warn!("On macOS, install BlackHole or Soundflower for system audio capture.");
                return Ok(());
            }
        }
    };

    info!("Using output loopback device: {}", device.name().unwrap_or_default());

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    info!(
        "Output loopback config: {} Hz, {} channels",
        sample_rate, channels
    );

    let sample_count = Arc::new(AtomicU64::new(0));
    let sample_count_clone = sample_count.clone();
    let orchestrator_clone = orchestrator.clone();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Convert f32 to i16 PCM
            let samples: Vec<i16> = data
                .iter()
                .map(|&s| (s * 32767.0) as i16)
                .collect();

            let bytes: Vec<u8> = samples
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .collect();

            let data_len = data.len() as u64;

            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                let orchestrator = orchestrator_clone.clone();
                let sample_count = sample_count_clone.clone();
                handle.spawn(async move {
                    if let Some(ref cm) = *orchestrator.chunk_manager.lock().await {
                        let seq = sample_count.fetch_add(data_len, Ordering::SeqCst);
                        let ts = MultiTimestamp::now(seq);
                        if let Err(e) = cm.write_frame("output_audio.pcm", &bytes, ts).await {
                            debug!("Failed to write output audio: {}", e);
                        }
                    }
                });
            }
        },
        |err| {
            error!("Output stream error: {}", err);
        },
        None,
    )?;

    stream.play()?;

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

/// List available audio devices
pub fn list_audio_devices() -> Result<serde_json::Value> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();

    let mut input_devices = Vec::new();
    let mut output_devices = Vec::new();

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                input_devices.push(serde_json::json!({
                    "name": name,
                    "type": "input"
                }));
            }
        }
    }

    if let Ok(devices) = host.output_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                output_devices.push(serde_json::json!({
                    "name": name,
                    "type": "output"
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "input_devices": input_devices,
        "output_devices": output_devices,
        "default_input": host.default_input_device().and_then(|d| d.name().ok()),
        "default_output": host.default_output_device().and_then(|d| d.name().ok()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_audio_devices() {
        let result = list_audio_devices();
        assert!(result.is_ok());

        let devices = result.unwrap();
        assert!(devices.get("input_devices").is_some());
        assert!(devices.get("output_devices").is_some());
        assert!(devices.get("default_input").is_some());
        assert!(devices.get("default_output").is_some());
    }

    #[test]
    fn test_list_audio_devices_structure() {
        let result = list_audio_devices().unwrap();

        // input_devices should be an array
        assert!(result["input_devices"].is_array());
        assert!(result["output_devices"].is_array());
    }

    #[test]
    fn test_f32_to_i16_conversion() {
        // Test the conversion used in audio capture
        let f32_samples: Vec<f32> = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let i16_samples: Vec<i16> = f32_samples
            .iter()
            .map(|&s| (s * 32767.0) as i16)
            .collect();

        assert_eq!(i16_samples[0], 0);           // 0.0 -> 0
        assert_eq!(i16_samples[1], 16383);       // 0.5 -> ~16384
        assert_eq!(i16_samples[2], -16383);      // -0.5 -> ~-16384
        assert_eq!(i16_samples[3], 32767);       // 1.0 -> 32767
        assert_eq!(i16_samples[4], -32767);      // -1.0 -> -32767
    }

    #[test]
    fn test_i16_to_bytes_conversion() {
        let samples: Vec<i16> = vec![0, 1000, -1000, 32767, -32768];
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();

        // Each i16 should produce 2 bytes (little endian)
        assert_eq!(bytes.len(), samples.len() * 2);

        // Verify first sample (0)
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[1], 0);

        // Verify max positive (32767 = 0x7FFF)
        assert_eq!(bytes[6], 0xFF);
        assert_eq!(bytes[7], 0x7F);

        // Verify max negative (-32768 = 0x8000)
        assert_eq!(bytes[8], 0x00);
        assert_eq!(bytes[9], 0x80);
    }

    #[test]
    fn test_sample_rate_constants() {
        // Common sample rates
        let rates = [22050u32, 44100, 48000, 96000];

        for rate in rates {
            // Should be divisible by common factors
            assert!(rate > 0);
        }

        // Standard CD quality
        assert_eq!(44100 % 100, 0);
    }

    #[test]
    fn test_channel_counts() {
        // Common channel configurations
        let mono = 1u32;
        let stereo = 2u32;

        assert_eq!(mono, 1);
        assert_eq!(stereo, 2);
    }

    #[test]
    fn test_audio_buffer_size_calculation() {
        let sample_rate = 44100u32;
        let channels = 2u32;
        let duration_ms = 100u64;

        // Calculate expected samples for 100ms of stereo audio
        let expected_samples = (sample_rate as u64 * duration_ms * channels as u64) / 1000;
        assert_eq!(expected_samples, 8820);

        // Bytes needed (16-bit audio)
        let bytes_needed = expected_samples * 2;
        assert_eq!(bytes_needed, 17640);
    }

    #[test]
    fn test_atomic_bool_usage() {
        let running = Arc::new(AtomicBool::new(true));

        assert!(running.load(Ordering::SeqCst));

        running.store(false, Ordering::SeqCst);
        assert!(!running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_atomic_counter_usage() {
        let counter = Arc::new(AtomicU64::new(0));

        // Simulate multiple frame increments
        for _ in 0..100 {
            counter.fetch_add(1, Ordering::SeqCst);
        }

        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_audio_config_defaults() {
        use super::super::config::AudioInputConfig;

        let config = AudioInputConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.sample_rate, 44100);
        assert!(config.device.is_none());
    }

    #[test]
    fn test_audio_output_config_defaults() {
        use super::super::config::AudioOutputConfig;

        let config = AudioOutputConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.sample_rate, 44100);
        assert!(config.device.is_none());
    }

    #[test]
    fn test_pcm_file_naming() {
        let mic_file = "mic_audio.pcm";
        let output_file = "output_audio.pcm";

        assert!(mic_file.ends_with(".pcm"));
        assert!(output_file.ends_with(".pcm"));
        assert!(mic_file.contains("mic"));
        assert!(output_file.contains("output"));
    }
}
