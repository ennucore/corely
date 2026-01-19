//! Video encoder using FFmpeg as a subprocess.
//!
//! Encodes raw frames to H.264 video in MP4 container.

use anyhow::{anyhow, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use tracing::{debug, error, info, warn};

/// Video encoder that pipes frames to FFmpeg
pub struct VideoEncoder {
    process: Child,
    width: u32,
    height: u32,
    fps: u32,
    frames_written: u64,
}

impl VideoEncoder {
    /// Create a new encoder for a given output file
    pub fn new(output_path: &Path, width: u32, height: u32, fps: u32, quality: u32) -> Result<Self> {
        // CRF value: 0 (lossless) to 51 (worst quality)
        // Map quality 0-100 to CRF 51-18
        let crf = 51 - (quality as f32 * 0.33) as u32;

        let process = Command::new("ffmpeg")
            .args([
                "-y",                           // Overwrite output
                "-f", "rawvideo",               // Input format
                "-pix_fmt", "rgb24",            // Pixel format
                "-s", &format!("{}x{}", width, height),  // Size
                "-r", &fps.to_string(),         // Frame rate
                "-i", "-",                      // Input from stdin
                "-c:v", "libx264",              // H.264 codec
                "-preset", "ultrafast",         // Encoding speed
                "-crf", &crf.to_string(),       // Quality
                "-pix_fmt", "yuv420p",          // Output pixel format
                output_path.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            process,
            width,
            height,
            fps,
            frames_written: 0,
        })
    }

    /// Create encoder for JPEG input frames
    pub fn new_jpeg_input(output_path: &Path, fps: u32, quality: u32) -> Result<Self> {
        let crf = 51 - (quality as f32 * 0.33) as u32;

        let process = Command::new("ffmpeg")
            .args([
                "-y",                           // Overwrite output
                "-f", "image2pipe",             // Input format for piped images
                "-framerate", &fps.to_string(), // Frame rate
                "-i", "-",                      // Input from stdin
                "-c:v", "libx264",              // H.264 codec
                "-preset", "ultrafast",         // Encoding speed
                "-crf", &crf.to_string(),       // Quality
                "-pix_fmt", "yuv420p",          // Output pixel format
                output_path.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            process,
            width: 0,
            height: 0,
            fps,
            frames_written: 0,
        })
    }

    /// Write a raw RGB frame
    pub fn write_frame(&mut self, frame_data: &[u8]) -> Result<()> {
        if let Some(stdin) = self.process.stdin.as_mut() {
            stdin.write_all(frame_data)?;
            self.frames_written += 1;
            Ok(())
        } else {
            Err(anyhow!("FFmpeg stdin not available"))
        }
    }

    /// Write a JPEG frame
    pub fn write_jpeg_frame(&mut self, jpeg_data: &[u8]) -> Result<()> {
        if let Some(stdin) = self.process.stdin.as_mut() {
            stdin.write_all(jpeg_data)?;
            self.frames_written += 1;
            Ok(())
        } else {
            Err(anyhow!("FFmpeg stdin not available"))
        }
    }

    /// Finalize the video
    pub fn finish(mut self) -> Result<u64> {
        // Close stdin to signal EOF
        drop(self.process.stdin.take());

        // Wait for FFmpeg to finish
        let output = self.process.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("FFmpeg warning/error: {}", stderr);
        }

        Ok(self.frames_written)
    }

    /// Get number of frames written
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
}

/// Audio encoder for PCM to AAC/MP3
pub struct AudioEncoder {
    process: Child,
    sample_rate: u32,
    channels: u32,
    samples_written: u64,
}

impl AudioEncoder {
    /// Create a new audio encoder
    pub fn new(output_path: &Path, sample_rate: u32, channels: u32) -> Result<Self> {
        let process = Command::new("ffmpeg")
            .args([
                "-y",                           // Overwrite output
                "-f", "s16le",                  // Input format (16-bit signed LE)
                "-ar", &sample_rate.to_string(), // Sample rate
                "-ac", &channels.to_string(),   // Channels
                "-i", "-",                      // Input from stdin
                "-c:a", "aac",                  // AAC codec
                "-b:a", "128k",                 // Bitrate
                output_path.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            process,
            sample_rate,
            channels,
            samples_written: 0,
        })
    }

    /// Write audio samples (PCM s16le format)
    pub fn write_samples(&mut self, samples: &[i16]) -> Result<()> {
        if let Some(stdin) = self.process.stdin.as_mut() {
            let bytes: Vec<u8> = samples
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .collect();
            stdin.write_all(&bytes)?;
            self.samples_written += samples.len() as u64;
            Ok(())
        } else {
            Err(anyhow!("FFmpeg stdin not available"))
        }
    }

    /// Finalize the audio
    pub fn finish(mut self) -> Result<u64> {
        drop(self.process.stdin.take());
        let output = self.process.wait_with_output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("FFmpeg audio warning/error: {}", stderr);
        }

        Ok(self.samples_written)
    }
}

/// Check if FFmpeg is available
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_ffmpeg_available() {
        let available = is_ffmpeg_available();
        println!("FFmpeg available: {}", available);
        // Note: This test is informational - doesn't fail if FFmpeg not installed
    }

    #[test]
    fn test_video_encoder_creation_without_ffmpeg() {
        // This tests the encoder creation - it will fail if ffmpeg is not installed
        // which is expected in some test environments
        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test_output.mp4");

        let result = VideoEncoder::new(&output_path, 640, 480, 30, 80);

        // If FFmpeg is available, this should succeed
        if is_ffmpeg_available() {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_video_encoder_jpeg_input_creation() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test_output.mp4");

        let result = VideoEncoder::new_jpeg_input(&output_path, 5, 80);
        assert!(result.is_ok());
    }

    #[test]
    fn test_audio_encoder_creation() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test_audio.aac");

        let result = AudioEncoder::new(&output_path, 44100, 2);
        assert!(result.is_ok());

        let encoder = result.unwrap();
        assert_eq!(encoder.sample_rate, 44100);
        assert_eq!(encoder.channels, 2);
        assert_eq!(encoder.samples_written, 0);
    }

    #[test]
    fn test_video_encoder_write_frames() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test_video.mp4");

        let mut encoder = VideoEncoder::new(&output_path, 320, 240, 10, 80).unwrap();

        // Create some dummy RGB frames (320x240x3 bytes each)
        let frame_size = 320 * 240 * 3;
        let frame = vec![128u8; frame_size];

        // Write 10 frames
        for _ in 0..10 {
            encoder.write_frame(&frame).unwrap();
        }

        assert_eq!(encoder.frames_written(), 10);

        let frames = encoder.finish().unwrap();
        assert_eq!(frames, 10);

        // Verify output file was created
        assert!(output_path.exists());
        assert!(fs::metadata(&output_path).unwrap().len() > 0);
    }

    #[test]
    fn test_audio_encoder_write_samples() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test_audio.aac");

        let mut encoder = AudioEncoder::new(&output_path, 44100, 2).unwrap();

        // Create 1 second of stereo audio (44100 samples * 2 channels)
        let samples: Vec<i16> = (0..44100 * 2)
            .map(|i| ((i as f32 * 0.1).sin() * 16000.0) as i16)
            .collect();

        encoder.write_samples(&samples).unwrap();

        assert_eq!(encoder.samples_written, samples.len() as u64);

        let total = encoder.finish().unwrap();
        assert_eq!(total, samples.len() as u64);

        assert!(output_path.exists());
    }

    #[test]
    fn test_video_encoder_empty_finish() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("empty_video.mp4");

        let encoder = VideoEncoder::new(&output_path, 320, 240, 30, 80).unwrap();
        let frames = encoder.finish().unwrap();

        assert_eq!(frames, 0);
    }

    #[test]
    fn test_video_encoder_quality_settings() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();

        // Test different quality levels
        for quality in [20, 50, 80, 100] {
            let output_path = dir.path().join(format!("video_q{}.mp4", quality));
            let result = VideoEncoder::new(&output_path, 160, 120, 5, quality);
            assert!(result.is_ok(), "Failed to create encoder with quality {}", quality);
        }
    }

    #[test]
    fn test_video_encoder_various_resolutions() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let resolutions = [
            (160, 120),
            (320, 240),
            (640, 480),
            (1280, 720),
        ];

        for (width, height) in resolutions {
            let output_path = dir.path().join(format!("video_{}x{}.mp4", width, height));
            let result = VideoEncoder::new(&output_path, width, height, 10, 80);
            assert!(result.is_ok(), "Failed for resolution {}x{}", width, height);
        }
    }

    #[test]
    fn test_audio_encoder_various_sample_rates() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let sample_rates = [22050, 44100, 48000];

        for rate in sample_rates {
            let output_path = dir.path().join(format!("audio_{}.aac", rate));
            let result = AudioEncoder::new(&output_path, rate, 2);
            assert!(result.is_ok(), "Failed for sample rate {}", rate);
        }
    }

    #[test]
    fn test_audio_encoder_mono_stereo() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();

        // Mono
        let mono_path = dir.path().join("mono.aac");
        let mono_result = AudioEncoder::new(&mono_path, 44100, 1);
        assert!(mono_result.is_ok());

        // Stereo
        let stereo_path = dir.path().join("stereo.aac");
        let stereo_result = AudioEncoder::new(&stereo_path, 44100, 2);
        assert!(stereo_result.is_ok());
    }

    #[test]
    fn test_video_encoder_frame_counter() {
        if !is_ffmpeg_available() {
            println!("Skipping test - FFmpeg not available");
            return;
        }

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("counter_test.mp4");

        let mut encoder = VideoEncoder::new(&output_path, 64, 64, 30, 50).unwrap();
        let frame = vec![0u8; 64 * 64 * 3];

        assert_eq!(encoder.frames_written(), 0);

        for i in 1..=5 {
            encoder.write_frame(&frame).unwrap();
            assert_eq!(encoder.frames_written(), i);
        }
    }
}
