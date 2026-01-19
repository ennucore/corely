//! Camera capture functionality.
//!
//! Uses platform-specific tools for camera access.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::process::Command;
use tracing::debug;

/// Capture a frame from the camera.
pub async fn capture(device_index: Option<u32>) -> Result<Value> {
    debug!("Capturing camera, device_index: {:?}", device_index);

    #[cfg(target_os = "macos")]
    {
        macos_capture(device_index).await
    }

    #[cfg(target_os = "linux")]
    {
        linux_capture(device_index).await
    }

    #[cfg(target_os = "windows")]
    {
        windows_capture(device_index).await
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(anyhow!("Camera capture not supported on this platform"))
    }
}

#[cfg(target_os = "macos")]
async fn macos_capture(_device_index: Option<u32>) -> Result<Value> {
    use std::fs;

    let temp_path = "/tmp/corely_camera.jpg";

    // Use imagesnap if available
    let output = Command::new("imagesnap")
        .args(["-q", temp_path])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let data = fs::read(temp_path)?;
            let _ = fs::remove_file(temp_path);
            let base64_data = BASE64.encode(&data);

            Ok(json!({
                "width": 0,
                "height": 0,
                "format": "jpeg",
                "data": base64_data,
            }))
        }
        _ => Err(anyhow!("Camera capture failed. Install imagesnap: brew install imagesnap")),
    }
}

#[cfg(target_os = "linux")]
async fn linux_capture(device_index: Option<u32>) -> Result<Value> {
    use std::fs;

    let device = format!("/dev/video{}", device_index.unwrap_or(0));
    let temp_path = "/tmp/corely_camera.jpg";

    // Use fswebcam if available
    let output = Command::new("fswebcam")
        .args(["-d", &device, "-r", "1280x720", "--no-banner", temp_path])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let data = fs::read(temp_path)?;
            let _ = fs::remove_file(temp_path);
            let base64_data = BASE64.encode(&data);

            Ok(json!({
                "width": 1280,
                "height": 720,
                "format": "jpeg",
                "data": base64_data,
            }))
        }
        _ => Err(anyhow!("Camera capture failed. Install fswebcam: sudo apt install fswebcam")),
    }
}

#[cfg(target_os = "windows")]
async fn windows_capture(_device_index: Option<u32>) -> Result<Value> {
    Err(anyhow!("Camera capture on Windows requires additional setup"))
}

/// List available camera devices.
pub async fn list_devices() -> Result<Value> {
    debug!("Listing camera devices");

    #[cfg(target_os = "macos")]
    {
        // Use system_profiler
        let output = Command::new("system_profiler")
            .args(["SPCameraDataType", "-json"])
            .output()?;

        if output.status.success() {
            if let Ok(data) = serde_json::from_slice::<Value>(&output.stdout) {
                let devices: Vec<Value> = data
                    .get("SPCameraDataType")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .enumerate()
                            .map(|(i, d)| {
                                json!({
                                    "index": i,
                                    "name": d.get("_name").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(json!({
                    "devices": devices,
                    "count": devices.len(),
                }));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Check /dev/video* devices
        let mut devices = Vec::new();
        for i in 0..10 {
            let path = format!("/dev/video{}", i);
            if std::path::Path::new(&path).exists() {
                devices.push(json!({
                    "index": i,
                    "name": format!("Video Device {}", i),
                    "path": path,
                }));
            }
        }

        return Ok(json!({
            "devices": devices,
            "count": devices.len(),
        }));
    }

    // Fallback
    Ok(json!({
        "devices": [],
        "count": 0,
    }))
}
