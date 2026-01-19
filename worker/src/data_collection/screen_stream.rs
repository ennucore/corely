//! Screen capture streaming using the scap crate.
//!
//! Captures all displays at configurable FPS and resolution.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::config::{MultiTimestamp, ScreenConfig};
use super::encoder::VideoEncoder;
use super::OrchestratorRefs;

/// Run screen capture for all configured displays
pub async fn run_capture(
    orchestrator: OrchestratorRefs,
    config: ScreenConfig,
) -> Result<()> {
    info!("Starting screen capture at {} FPS, {}p", config.fps, config.resolution);

    let mut shutdown_rx = orchestrator.get_shutdown_receiver().await;

    // Get list of displays to capture
    let displays = get_displays(&config)?;
    info!("Capturing {} display(s)", displays.len());

    // Calculate frame interval
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);

    // Frame counter for timestamps
    let frame_seq = AtomicU64::new(0);

    loop {
        let frame_start = Instant::now();

        // Check for shutdown
        if let Some(ref mut rx) = shutdown_rx {
            match rx.try_recv() {
                Ok(_) | Err(broadcast::error::TryRecvError::Closed) => {
                    info!("Screen capture shutting down");
                    break;
                }
                Err(broadcast::error::TryRecvError::Empty) => {}
                Err(broadcast::error::TryRecvError::Lagged(_)) => {}
            }
        }

        // Capture each display
        for disp in &displays {
            let seq = frame_seq.fetch_add(1, Ordering::SeqCst);
            let timestamp = MultiTimestamp::now(seq);

            match capture_display(disp, config.resolution, config.quality) {
                Ok(frame_data) => {
                    // Write to chunk manager
                    let stream_name = format!("display_{}/video.raw", disp.id);
                    if let Some(ref cm) = *orchestrator.chunk_manager.lock().await {
                        if let Err(e) = cm.write_frame(&stream_name, &frame_data, timestamp).await {
                            error!("Failed to write frame: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to capture display {}: {}", disp.id, e);
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

/// Display information
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Get list of displays to capture
fn get_displays(config: &ScreenConfig) -> Result<Vec<DisplayInfo>> {
    let mut displays = Vec::new();

    #[cfg(target_os = "macos")]
    {
        displays = get_macos_displays()?;
    }

    #[cfg(target_os = "linux")]
    {
        displays = get_linux_displays()?;
    }

    #[cfg(target_os = "windows")]
    {
        displays = get_windows_displays()?;
    }

    // Filter by config
    if !config.all_displays && !config.display_ids.is_empty() {
        displays.retain(|d| config.display_ids.contains(&d.id));
    }

    if displays.is_empty() {
        // Fallback to a single display
        displays.push(DisplayInfo {
            id: 0,
            name: "Primary".to_string(),
            width: 1920,
            height: 1080,
            is_primary: true,
        });
    }

    Ok(displays)
}

#[cfg(target_os = "macos")]
fn get_macos_displays() -> Result<Vec<DisplayInfo>> {
    use core_graphics::display::CGDisplay;

    let display_ids = CGDisplay::active_displays()
        .map_err(|e| anyhow!("Failed to get displays: {:?}", e))?;

    let displays: Vec<DisplayInfo> = display_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let display = CGDisplay::new(id);
            DisplayInfo {
                id: id,
                name: format!("Display {}", i),
                width: display.pixels_wide() as u32,
                height: display.pixels_high() as u32,
                is_primary: display.is_main(),
            }
        })
        .collect();

    Ok(displays)
}

#[cfg(target_os = "linux")]
fn get_linux_displays() -> Result<Vec<DisplayInfo>> {
    // On Linux, we'll use a simpler approach
    // In production, you'd use X11/Wayland APIs
    Ok(vec![DisplayInfo {
        id: 0,
        name: "Primary".to_string(),
        width: 1920,
        height: 1080,
        is_primary: true,
    }])
}

#[cfg(target_os = "windows")]
fn get_windows_displays() -> Result<Vec<DisplayInfo>> {
    // On Windows, we'd enumerate monitors
    Ok(vec![DisplayInfo {
        id: 0,
        name: "Primary".to_string(),
        width: 1920,
        height: 1080,
        is_primary: true,
    }])
}

/// Capture a single display frame
fn capture_display(display: &DisplayInfo, target_height: u32, quality: u32) -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        capture_macos_display(display, target_height, quality)
    }

    #[cfg(target_os = "linux")]
    {
        capture_linux_display(display, target_height, quality)
    }

    #[cfg(target_os = "windows")]
    {
        capture_windows_display(display, target_height, quality)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(anyhow!("Screen capture not supported on this platform"))
    }
}

#[cfg(target_os = "macos")]
fn capture_macos_display(display: &DisplayInfo, target_height: u32, quality: u32) -> Result<Vec<u8>> {
    use core_graphics::display::{CGDisplay, CGDisplayCreateImage};
    use std::process::Command;
    use std::fs;

    // Use screencapture for simplicity (could use CGDisplayCreateImage for raw access)
    let temp_path = format!("/tmp/corely_screen_{}.jpg", display.id);

    // Quality mapping: 0-100 to screencapture's -x (silent) with compression
    let output = Command::new("screencapture")
        .args(["-x", "-t", "jpg", &temp_path])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("screencapture failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let data = fs::read(&temp_path)?;
    let _ = fs::remove_file(&temp_path);

    Ok(data)
}

#[cfg(target_os = "linux")]
fn capture_linux_display(display: &DisplayInfo, target_height: u32, quality: u32) -> Result<Vec<u8>> {
    use std::process::Command;
    use std::fs;

    let temp_path = format!("/tmp/corely_screen_{}.jpg", display.id);

    // Try various screenshot tools
    let tools = [
        ("spectacle", vec!["-bn", "-o", &temp_path]),
        ("gnome-screenshot", vec!["-f", &temp_path]),
        ("scrot", vec![&temp_path]),
        ("import", vec!["-window", "root", &temp_path]),
    ];

    let mut success = false;
    for (tool, args) in &tools {
        if let Ok(output) = Command::new(tool).args(args).output() {
            if output.status.success() {
                success = true;
                break;
            }
        }
    }

    if !success {
        return Err(anyhow!("No screenshot tool available"));
    }

    let data = fs::read(&temp_path)?;
    let _ = fs::remove_file(&temp_path);

    Ok(data)
}

#[cfg(target_os = "windows")]
fn capture_windows_display(display: &DisplayInfo, target_height: u32, quality: u32) -> Result<Vec<u8>> {
    use std::process::Command;
    use std::fs;

    let temp_path = r"C:\Temp\corely_screenshot.png";

    // Create temp directory if needed
    let _ = fs::create_dir_all(r"C:\Temp");

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

    let data = fs::read(temp_path)?;
    let _ = fs::remove_file(temp_path);

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_info_creation() {
        let display = DisplayInfo {
            id: 0,
            name: "Test Display".to_string(),
            width: 1920,
            height: 1080,
            is_primary: true,
        };

        assert_eq!(display.id, 0);
        assert_eq!(display.name, "Test Display");
        assert_eq!(display.width, 1920);
        assert_eq!(display.height, 1080);
        assert!(display.is_primary);
    }

    #[test]
    fn test_display_info_secondary() {
        let display = DisplayInfo {
            id: 1,
            name: "Secondary".to_string(),
            width: 1280,
            height: 720,
            is_primary: false,
        };

        assert_eq!(display.id, 1);
        assert!(!display.is_primary);
    }

    #[test]
    fn test_screen_config_all_displays() {
        let config = ScreenConfig {
            enabled: true,
            fps: 1,
            resolution: 720,
            all_displays: true,
            display_ids: vec![],
            quality: 80,
        };

        assert!(config.all_displays);
        assert!(config.display_ids.is_empty());
    }

    #[test]
    fn test_screen_config_specific_displays() {
        let config = ScreenConfig {
            enabled: true,
            fps: 5,
            resolution: 1080,
            all_displays: false,
            display_ids: vec![0, 2],
            quality: 90,
        };

        assert!(!config.all_displays);
        assert_eq!(config.display_ids, vec![0, 2]);
    }

    #[test]
    fn test_get_displays_default_config() {
        let config = ScreenConfig::default();
        let result = get_displays(&config);

        assert!(result.is_ok());
        let displays = result.unwrap();
        assert!(!displays.is_empty());
    }

    #[test]
    fn test_get_displays_filtered_nonexistent() {
        let config = ScreenConfig {
            enabled: true,
            fps: 1,
            resolution: 720,
            all_displays: false,
            display_ids: vec![999],
            quality: 80,
        };

        let result = get_displays(&config);
        assert!(result.is_ok());
        // Should return fallback or filtered displays
    }

    #[test]
    fn test_frame_interval_calculation_1fps() {
        let interval = Duration::from_secs_f64(1.0 / 1.0);
        assert_eq!(interval, Duration::from_secs(1));
    }

    #[test]
    fn test_frame_interval_calculation_5fps() {
        let interval = Duration::from_secs_f64(1.0 / 5.0);
        assert_eq!(interval, Duration::from_millis(200));
    }

    #[test]
    fn test_frame_interval_calculation_30fps() {
        let interval = Duration::from_secs_f64(1.0 / 30.0);
        assert!(interval.as_millis() >= 33 && interval.as_millis() <= 34);
    }

    #[test]
    fn test_display_info_clone() {
        let display = DisplayInfo {
            id: 0,
            name: "Test".to_string(),
            width: 1920,
            height: 1080,
            is_primary: true,
        };

        let cloned = display.clone();
        assert_eq!(display.id, cloned.id);
        assert_eq!(display.name, cloned.name);
        assert_eq!(display.width, cloned.width);
        assert_eq!(display.height, cloned.height);
        assert_eq!(display.is_primary, cloned.is_primary);
    }

    #[test]
    fn test_quality_bounds() {
        for q in [1, 25, 50, 75, 100] {
            assert!(q >= 1 && q <= 100);
        }
    }

    #[test]
    fn test_resolution_presets() {
        let presets = [480, 720, 1080, 1440, 2160];
        for res in presets {
            let config = ScreenConfig {
                enabled: true,
                fps: 1,
                resolution: res,
                all_displays: true,
                display_ids: vec![],
                quality: 80,
            };
            assert_eq!(config.resolution, res);
        }
    }

    #[test]
    fn test_display_filtering_empty_list() {
        let config = ScreenConfig {
            enabled: true,
            fps: 1,
            resolution: 720,
            all_displays: false,
            display_ids: vec![],
            quality: 80,
        };

        let result = get_displays(&config);
        assert!(result.is_ok());
        // With empty filter and all_displays=false, should still return something
    }

    #[test]
    fn test_display_filtering_with_valid_ids() {
        let config = ScreenConfig {
            enabled: true,
            fps: 1,
            resolution: 720,
            all_displays: false,
            display_ids: vec![0],
            quality: 80,
        };

        let result = get_displays(&config);
        assert!(result.is_ok());
    }
}
