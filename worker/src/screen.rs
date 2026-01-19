//! Screen capture functionality.
//!
//! Uses platform-specific APIs for screen capture.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::process::Command;
use tracing::debug;

/// Capture the screen and return as base64 PNG.
pub async fn capture(display_id: Option<u32>) -> Result<Value> {
    debug!("Capturing screen, display_id: {:?}", display_id);

    #[cfg(target_os = "macos")]
    {
        macos_capture(display_id).await
    }

    #[cfg(target_os = "linux")]
    {
        linux_capture(display_id).await
    }

    #[cfg(target_os = "windows")]
    {
        windows_capture(display_id).await
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(anyhow!("Screen capture not supported on this platform"))
    }
}

#[cfg(target_os = "macos")]
async fn macos_capture(_display_id: Option<u32>) -> Result<Value> {
    use std::fs;
    use std::path::Path;

    let temp_path = "/tmp/corely_screenshot.png";

    // Use screencapture command
    let output = Command::new("screencapture")
        .args(["-x", "-t", "png", temp_path])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Read the file
    let data = fs::read(temp_path)?;
    let _ = fs::remove_file(temp_path);

    // Get dimensions using sips
    let sips_output = Command::new("sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight", temp_path])
        .output();

    let (width, height) = if let Ok(out) = sips_output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut w = 0u32;
        let mut h = 0u32;
        for line in stdout.lines() {
            if line.contains("pixelWidth") {
                w = line.split_whitespace().last().and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            if line.contains("pixelHeight") {
                h = line.split_whitespace().last().and_then(|s| s.parse().ok()).unwrap_or(0);
            }
        }
        (w, h)
    } else {
        (0, 0)
    };

    let base64_data = BASE64.encode(&data);

    Ok(json!({
        "width": width,
        "height": height,
        "format": "png",
        "data": base64_data,
    }))
}

#[cfg(target_os = "linux")]
async fn linux_capture(_display_id: Option<u32>) -> Result<Value> {
    use std::fs;

    let temp_path = "/tmp/corely_screenshot.png";

    // Remove any existing screenshot file
    let _ = fs::remove_file(temp_path);

    // First, try to find a graphical session and capture via Wayland/KDE tools
    // This is needed when running as root to capture from user's desktop
    if let Ok(result) = try_wayland_capture(temp_path) {
        if result {
            if let Ok(data) = fs::read(temp_path) {
                let _ = fs::remove_file(temp_path);
                let base64_data = BASE64.encode(&data);
                return Ok(json!({
                    "width": 0,
                    "height": 0,
                    "format": "png",
                    "data": base64_data,
                }));
            }
        }
    }

    // Fall back to X11 screenshot tools
    let tools = [
        ("gnome-screenshot", vec!["-f", temp_path]),
        ("scrot", vec![temp_path]),
        ("import", vec!["-window", "root", temp_path]),
    ];

    let mut success = false;
    for (tool, args) in &tools {
        if Command::new(tool).args(args).output().map(|o| o.status.success()).unwrap_or(false) {
            success = true;
            break;
        }
    }

    if !success {
        return Err(anyhow!("No screenshot tool available (tried spectacle, gnome-screenshot, scrot, import)"));
    }

    let data = fs::read(temp_path)?;
    let _ = fs::remove_file(temp_path);

    let base64_data = BASE64.encode(&data);

    Ok(json!({
        "width": 0,
        "height": 0,
        "format": "png",
        "data": base64_data,
    }))
}

#[cfg(target_os = "linux")]
fn try_wayland_capture(temp_path: &str) -> Result<bool> {
    // Find users with graphical sessions using loginctl
    let output = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let sessions_output = String::from_utf8_lossy(&output.stdout);

    // Parse session info and find graphical sessions
    for line in sessions_output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let session_id = parts[0];
        let uid: u32 = parts[1].parse().unwrap_or(0);
        let username = parts[2];

        // Skip root sessions
        if uid == 0 {
            continue;
        }

        // Check if this is a graphical session
        let session_type = Command::new("loginctl")
            .args(["show-session", session_id, "-p", "Type", "--value"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        if session_type != "wayland" && session_type != "x11" {
            continue;
        }

        debug!("Found graphical session: {} for user {} (type: {})", session_id, username, session_type);

        // Try to capture using spectacle (KDE) as this user
        let runtime_dir = format!("/run/user/{}", uid);
        let dbus_addr = format!("unix:path={}/bus", runtime_dir);

        // Build environment for the screenshot command
        let env_vars = format!(
            "XDG_RUNTIME_DIR={} DBUS_SESSION_BUS_ADDRESS={} QT_QPA_PLATFORM=wayland WAYLAND_DISPLAY=wayland-0",
            runtime_dir, dbus_addr
        );

        // Try spectacle (KDE)
        let spectacle_result = Command::new("sudo")
            .args([
                "-u", username,
                "bash", "-c",
                &format!("{} spectacle -bn -o {}", env_vars, temp_path)
            ])
            .output();

        if let Ok(output) = spectacle_result {
            if output.status.success() && std::path::Path::new(temp_path).exists() {
                debug!("spectacle capture succeeded");
                return Ok(true);
            }
        }

        // Try gnome-screenshot for GNOME Wayland
        let gnome_result = Command::new("sudo")
            .args([
                "-u", username,
                "bash", "-c",
                &format!("{} gnome-screenshot -f {}", env_vars, temp_path)
            ])
            .output();

        if let Ok(output) = gnome_result {
            if output.status.success() && std::path::Path::new(temp_path).exists() {
                debug!("gnome-screenshot capture succeeded");
                return Ok(true);
            }
        }

        // Try grim for wlroots-based compositors
        let grim_result = Command::new("sudo")
            .args([
                "-u", username,
                "bash", "-c",
                &format!("{} grim {}", env_vars, temp_path)
            ])
            .output();

        if let Ok(output) = grim_result {
            if output.status.success() && std::path::Path::new(temp_path).exists() {
                debug!("grim capture succeeded");
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[cfg(target_os = "windows")]
async fn windows_capture(_display_id: Option<u32>) -> Result<Value> {
    // Use PowerShell to capture screen
    let ps_script = r#"
        Add-Type -AssemblyName System.Windows.Forms
        $screen = [System.Windows.Forms.Screen]::PrimaryScreen
        $bitmap = New-Object System.Drawing.Bitmap($screen.Bounds.Width, $screen.Bounds.Height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($screen.Bounds.Location, [System.Drawing.Point]::Empty, $screen.Bounds.Size)
        $bitmap.Save('C:\Temp\corely_screenshot.png')
        $graphics.Dispose()
        $bitmap.Dispose()
    "#;

    Command::new("powershell")
        .args(["-Command", ps_script])
        .output()?;

    let data = std::fs::read("C:\\Temp\\corely_screenshot.png")?;
    let _ = std::fs::remove_file("C:\\Temp\\corely_screenshot.png");

    let base64_data = BASE64.encode(&data);

    Ok(json!({
        "width": 0,
        "height": 0,
        "format": "png",
        "data": base64_data,
    }))
}

/// List available displays.
pub async fn list_displays() -> Result<Value> {
    debug!("Listing displays");

    #[cfg(target_os = "macos")]
    {
        // Use system_profiler on macOS
        let output = Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()?;

        if output.status.success() {
            if let Ok(data) = serde_json::from_slice::<Value>(&output.stdout) {
                let displays: Vec<Value> = data
                    .get("SPDisplaysDataType")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .enumerate()
                            .map(|(i, d)| {
                                json!({
                                    "id": i,
                                    "name": d.get("sppci_model").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "resolution": d.get("spdisplays_resolution").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(json!({
                    "displays": displays,
                    "count": displays.len(),
                }));
            }
        }
    }

    // Fallback
    Ok(json!({
        "displays": [{"id": 0, "name": "Primary Display"}],
        "count": 1,
    }))
}
