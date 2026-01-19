//! Camera capture streaming.
//!
//! Uses platform-specific APIs for camera capture.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::config::{CameraConfig, MultiTimestamp};
use super::OrchestratorRefs;

/// Run camera capture for all configured cameras
pub async fn run_capture(orchestrator: OrchestratorRefs, config: CameraConfig) -> Result<()> {
    info!("Starting camera capture at {} FPS", config.fps);

    let mut shutdown_rx = orchestrator.get_shutdown_receiver().await;

    // Get list of cameras
    let cameras = get_cameras(&config)?;
    info!("Capturing {} camera(s)", cameras.len());

    if cameras.is_empty() {
        warn!("No cameras found to capture");
        return Ok(());
    }

    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let frame_seq = AtomicU64::new(0);

    loop {
        let frame_start = Instant::now();

        // Check for shutdown
        if let Some(ref mut rx) = shutdown_rx {
            match rx.try_recv() {
                Ok(_) | Err(broadcast::error::TryRecvError::Closed) => {
                    info!("Camera capture shutting down");
                    break;
                }
                Err(broadcast::error::TryRecvError::Empty) => {}
                Err(broadcast::error::TryRecvError::Lagged(_)) => {}
            }
        }

        // Capture each camera
        for camera in &cameras {
            let seq = frame_seq.fetch_add(1, Ordering::SeqCst);
            let timestamp = MultiTimestamp::now(seq);

            match capture_camera(camera, config.resolution) {
                Ok(frame_data) => {
                    let stream_name = format!("camera_{}/video.raw", camera.index);
                    if let Some(ref cm) = *orchestrator.chunk_manager.lock().await {
                        if let Err(e) = cm.write_frame(&stream_name, &frame_data, timestamp).await {
                            error!("Failed to write camera frame: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to capture camera {}: {}", camera.index, e);
                }
            }
        }

        // Wait for next frame
        let elapsed = frame_start.elapsed();
        if elapsed < frame_interval {
            tokio::time::sleep(frame_interval - elapsed).await;
        }
    }

    Ok(())
}

/// Camera information
#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub index: u32,
    pub name: String,
    pub description: String,
}

/// Get list of cameras to capture
fn get_cameras(config: &CameraConfig) -> Result<Vec<CameraInfo>> {
    let mut cameras = list_cameras_internal()?;

    // Filter by config
    if !config.all_cameras && !config.camera_indices.is_empty() {
        cameras.retain(|c| config.camera_indices.contains(&c.index));
    }

    Ok(cameras)
}

fn list_cameras_internal() -> Result<Vec<CameraInfo>> {
    #[cfg(target_os = "macos")]
    {
        list_macos_cameras()
    }

    #[cfg(target_os = "linux")]
    {
        list_linux_cameras()
    }

    #[cfg(target_os = "windows")]
    {
        list_windows_cameras()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "macos")]
fn list_macos_cameras() -> Result<Vec<CameraInfo>> {
    use std::process::Command;

    // Use system_profiler to list cameras
    let output = Command::new("system_profiler")
        .args(["SPCameraDataType", "-json"])
        .output()?;

    if output.status.success() {
        if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            let cameras: Vec<CameraInfo> = data
                .get("SPCameraDataType")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, c)| CameraInfo {
                            index: i as u32,
                            name: c
                                .get("_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            description: c
                                .get("spcamera_model-id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            return Ok(cameras);
        }
    }

    // Fallback: assume at least one camera
    Ok(vec![CameraInfo {
        index: 0,
        name: "FaceTime Camera".to_string(),
        description: "Built-in camera".to_string(),
    }])
}

#[cfg(target_os = "linux")]
fn list_linux_cameras() -> Result<Vec<CameraInfo>> {
    use std::fs;
    use std::path::Path;

    let mut cameras = Vec::new();

    // List /dev/video* devices
    if let Ok(entries) = fs::read_dir("/dev") {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("video") {
                if let Ok(index) = name[5..].parse::<u32>() {
                    cameras.push(CameraInfo {
                        index,
                        name: format!("/dev/{}", name),
                        description: "V4L2 video device".to_string(),
                    });
                }
            }
        }
    }

    Ok(cameras)
}

#[cfg(target_os = "windows")]
fn list_windows_cameras() -> Result<Vec<CameraInfo>> {
    // On Windows, we'd use DirectShow or Media Foundation to enumerate devices
    // For now, assume one camera
    Ok(vec![CameraInfo {
        index: 0,
        name: "Default Camera".to_string(),
        description: "Default video capture device".to_string(),
    }])
}

/// Capture a frame from a camera
fn capture_camera(camera: &CameraInfo, target_height: u32) -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        capture_macos_camera(camera, target_height)
    }

    #[cfg(target_os = "linux")]
    {
        capture_linux_camera(camera, target_height)
    }

    #[cfg(target_os = "windows")]
    {
        capture_windows_camera(camera, target_height)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(anyhow!("Camera capture not supported on this platform"))
    }
}

#[cfg(target_os = "macos")]
fn capture_macos_camera(camera: &CameraInfo, _target_height: u32) -> Result<Vec<u8>> {
    use std::fs;
    use std::process::Command;

    let temp_path = format!("/tmp/corely_camera_{}.jpg", camera.index);

    // Use imagesnap CLI tool (commonly available via Homebrew)
    let output = Command::new("imagesnap")
        .args(["-q", "-w", "0.1", &temp_path])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let data = fs::read(&temp_path)?;
            let _ = fs::remove_file(&temp_path);
            return Ok(data);
        }
    }

    // Fallback: use ffmpeg to capture from camera
    let output = Command::new("ffmpeg")
        .args([
            "-f",
            "avfoundation",
            "-framerate",
            "1",
            "-i",
            &format!("{}:", camera.index),
            "-frames:v",
            "1",
            "-y",
            &temp_path,
        ])
        .output()?;

    if output.status.success() {
        let data = fs::read(&temp_path)?;
        let _ = fs::remove_file(&temp_path);
        return Ok(data);
    }

    Err(anyhow!(
        "Failed to capture camera. Install imagesnap or ensure ffmpeg is available."
    ))
}

#[cfg(target_os = "linux")]
fn capture_linux_camera(camera: &CameraInfo, _target_height: u32) -> Result<Vec<u8>> {
    use std::fs;
    use std::process::Command;

    let temp_path = format!("/tmp/corely_camera_{}.jpg", camera.index);
    let device = format!("/dev/video{}", camera.index);

    // Use ffmpeg to capture from V4L2 device
    let output = Command::new("ffmpeg")
        .args([
            "-f",
            "v4l2",
            "-i",
            &device,
            "-frames:v",
            "1",
            "-y",
            &temp_path,
        ])
        .output()?;

    if output.status.success() {
        let data = fs::read(&temp_path)?;
        let _ = fs::remove_file(&temp_path);
        return Ok(data);
    }

    Err(anyhow!(
        "Failed to capture from camera: {}",
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[cfg(target_os = "windows")]
fn capture_windows_camera(camera: &CameraInfo, _target_height: u32) -> Result<Vec<u8>> {
    use std::fs;
    use std::process::Command;

    let temp_path = r"C:\Temp\corely_camera.jpg";

    // Use ffmpeg with DirectShow
    let output = Command::new("ffmpeg")
        .args([
            "-f",
            "dshow",
            "-i",
            "video=default",
            "-frames:v",
            "1",
            "-y",
            temp_path,
        ])
        .output()?;

    if output.status.success() {
        let data = fs::read(temp_path)?;
        let _ = fs::remove_file(temp_path);
        return Ok(data);
    }

    Err(anyhow!("Failed to capture camera"))
}

/// List available cameras (public API)
pub fn list_cameras() -> Result<serde_json::Value> {
    let cameras = list_cameras_internal()?;

    Ok(serde_json::json!({
        "cameras": cameras.iter().map(|c| serde_json::json!({
            "index": c.index,
            "name": c.name,
            "description": c.description,
        })).collect::<Vec<_>>(),
        "count": cameras.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_info_creation() {
        let camera = CameraInfo {
            index: 0,
            name: "Test Camera".to_string(),
            description: "A test camera device".to_string(),
        };

        assert_eq!(camera.index, 0);
        assert_eq!(camera.name, "Test Camera");
        assert_eq!(camera.description, "A test camera device");
    }

    #[test]
    fn test_camera_info_clone() {
        let camera = CameraInfo {
            index: 1,
            name: "FaceTime HD".to_string(),
            description: "Built-in camera".to_string(),
        };

        let cloned = camera.clone();
        assert_eq!(camera.index, cloned.index);
        assert_eq!(camera.name, cloned.name);
        assert_eq!(camera.description, cloned.description);
    }

    #[test]
    fn test_camera_info_debug() {
        let camera = CameraInfo {
            index: 2,
            name: "USB Webcam".to_string(),
            description: "External USB camera".to_string(),
        };

        let debug_str = format!("{:?}", camera);
        assert!(debug_str.contains("CameraInfo"));
        assert!(debug_str.contains("USB Webcam"));
    }

    #[test]
    fn test_camera_config_all_cameras() {
        let config = CameraConfig {
            enabled: true,
            fps: 5,
            resolution: 720,
            all_cameras: true,
            camera_indices: vec![],
            implies_mic: true,
        };

        assert!(config.all_cameras);
        assert!(config.camera_indices.is_empty());
        assert_eq!(config.fps, 5);
    }

    #[test]
    fn test_camera_config_specific_indices() {
        let config = CameraConfig {
            enabled: true,
            fps: 10,
            resolution: 1080,
            all_cameras: false,
            camera_indices: vec![0, 2],
            implies_mic: true,
        };

        assert!(!config.all_cameras);
        assert_eq!(config.camera_indices, vec![0, 2]);
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.fps, 5);
        assert_eq!(config.resolution, 480);
        assert!(config.all_cameras);
        assert!(config.camera_indices.is_empty());
        assert!(config.implies_mic);
    }

    #[test]
    fn test_get_cameras_all_cameras() {
        let config = CameraConfig {
            enabled: true,
            fps: 5,
            resolution: 720,
            all_cameras: true,
            camera_indices: vec![],
            implies_mic: true,
        };

        // This should not panic; result depends on system hardware
        let result = get_cameras(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_cameras_filtered() {
        let config = CameraConfig {
            enabled: true,
            fps: 5,
            resolution: 720,
            all_cameras: false,
            camera_indices: vec![0],
            implies_mic: true,
        };

        let result = get_cameras(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_cameras_nonexistent_index() {
        let config = CameraConfig {
            enabled: true,
            fps: 5,
            resolution: 720,
            all_cameras: false,
            camera_indices: vec![999],
            implies_mic: true,
        };

        let result = get_cameras(&config);
        assert!(result.is_ok());
        // Should return empty or filtered list
    }

    #[test]
    fn test_frame_interval_5fps() {
        let interval = Duration::from_secs_f64(1.0 / 5.0);
        assert_eq!(interval, Duration::from_millis(200));
    }

    #[test]
    fn test_frame_interval_10fps() {
        let interval = Duration::from_secs_f64(1.0 / 10.0);
        assert_eq!(interval, Duration::from_millis(100));
    }

    #[test]
    fn test_frame_interval_30fps() {
        let interval = Duration::from_secs_f64(1.0 / 30.0);
        assert!(interval.as_millis() >= 33 && interval.as_millis() <= 34);
    }

    #[test]
    fn test_frame_interval_1fps() {
        let interval = Duration::from_secs_f64(1.0 / 1.0);
        assert_eq!(interval, Duration::from_secs(1));
    }

    #[test]
    fn test_list_cameras_format() {
        let result = list_cameras();
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.get("cameras").is_some());
        assert!(json.get("count").is_some());
        assert!(json["cameras"].is_array());
    }

    #[test]
    fn test_list_cameras_json_structure() {
        let result = list_cameras().unwrap();

        if let Some(cameras) = result["cameras"].as_array() {
            for camera in cameras {
                assert!(camera.get("index").is_some());
                assert!(camera.get("name").is_some());
                assert!(camera.get("description").is_some());
            }
        }
    }

    #[test]
    fn test_camera_info_multiple_instances() {
        let cameras = vec![
            CameraInfo {
                index: 0,
                name: "Camera 0".to_string(),
                description: "First".to_string(),
            },
            CameraInfo {
                index: 1,
                name: "Camera 1".to_string(),
                description: "Second".to_string(),
            },
            CameraInfo {
                index: 2,
                name: "Camera 2".to_string(),
                description: "Third".to_string(),
            },
        ];

        assert_eq!(cameras.len(), 3);
        for (i, camera) in cameras.iter().enumerate() {
            assert_eq!(camera.index, i as u32);
        }
    }

    #[test]
    fn test_camera_filter_retain() {
        let mut cameras = vec![
            CameraInfo { index: 0, name: "A".to_string(), description: "".to_string() },
            CameraInfo { index: 1, name: "B".to_string(), description: "".to_string() },
            CameraInfo { index: 2, name: "C".to_string(), description: "".to_string() },
        ];

        let indices_to_keep = vec![0, 2];
        cameras.retain(|c| indices_to_keep.contains(&c.index));

        assert_eq!(cameras.len(), 2);
        assert_eq!(cameras[0].index, 0);
        assert_eq!(cameras[1].index, 2);
    }

    #[test]
    fn test_resolution_presets() {
        let presets = [480, 720, 1080, 1440];

        for res in presets {
            let config = CameraConfig {
                enabled: true,
                fps: 5,
                resolution: res,
                all_cameras: true,
                camera_indices: vec![],
                implies_mic: true,
            };
            assert_eq!(config.resolution, res);
        }
    }

    #[test]
    fn test_fps_values() {
        let fps_values = [1, 5, 10, 15, 30];

        for fps in fps_values {
            let config = CameraConfig {
                enabled: true,
                fps,
                resolution: 720,
                all_cameras: true,
                camera_indices: vec![],
                implies_mic: true,
            };
            assert_eq!(config.fps, fps);
        }
    }

    #[test]
    fn test_stream_name_format() {
        let camera = CameraInfo {
            index: 0,
            name: "Test".to_string(),
            description: "".to_string(),
        };

        let stream_name = format!("camera_{}/video.raw", camera.index);
        assert_eq!(stream_name, "camera_0/video.raw");
    }

    #[test]
    fn test_stream_name_multiple_cameras() {
        for i in 0..5 {
            let stream_name = format!("camera_{}/video.raw", i);
            assert!(stream_name.starts_with("camera_"));
            assert!(stream_name.ends_with("/video.raw"));
            assert!(stream_name.contains(&i.to_string()));
        }
    }

    #[test]
    fn test_atomic_frame_sequence() {
        let frame_seq = AtomicU64::new(0);

        for expected in 0..50 {
            let seq = frame_seq.fetch_add(1, Ordering::SeqCst);
            assert_eq!(seq, expected);
        }

        assert_eq!(frame_seq.load(Ordering::SeqCst), 50);
    }

    #[test]
    fn test_frame_timing_calculation() {
        let frame_interval = Duration::from_secs_f64(1.0 / 5.0);
        let start = Instant::now();

        // Simulate shorter elapsed time
        let elapsed = Duration::from_millis(50);

        if elapsed < frame_interval {
            let sleep_duration = frame_interval - elapsed;
            assert_eq!(sleep_duration, Duration::from_millis(150));
        }
    }

    #[test]
    fn test_camera_info_with_special_characters() {
        let camera = CameraInfo {
            index: 0,
            name: "USB Camera (HD Pro)".to_string(),
            description: "Model: ABC-123 / Rev: 2.0".to_string(),
        };

        assert!(camera.name.contains("("));
        assert!(camera.description.contains("/"));
    }

    #[test]
    fn test_empty_camera_indices_with_all_false() {
        let config = CameraConfig {
            enabled: true,
            fps: 5,
            resolution: 720,
            all_cameras: false,
            camera_indices: vec![],
            implies_mic: true,
        };

        // With all_cameras=false and empty indices, filtering should keep all
        // (because the condition is: !all_cameras && !indices.is_empty())
        let result = get_cameras(&config);
        assert!(result.is_ok());
    }
}
