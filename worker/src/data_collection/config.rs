//! Configuration structures for data collection.

use serde::{Deserialize, Serialize};

/// Main configuration for all data collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConfig {
    /// Screen capture configuration
    #[serde(default)]
    pub screen: ScreenConfig,

    /// Camera capture configuration
    #[serde(default)]
    pub camera: CameraConfig,

    /// Audio input (microphone) configuration
    #[serde(default)]
    pub audio_input: AudioInputConfig,

    /// Audio output (system loopback) configuration
    #[serde(default)]
    pub audio_output: AudioOutputConfig,

    /// Input logging (keystrokes/mouse) configuration
    #[serde(default)]
    pub input_logging: InputLoggingConfig,

    /// Directory sync configuration
    #[serde(default)]
    pub directory_sync: DirectorySyncConfig,

    /// Chunk duration in seconds (default: 60)
    #[serde(default = "default_chunk_duration")]
    pub chunk_duration_secs: u64,

    /// Output directory for collected data
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

fn default_chunk_duration() -> u64 {
    60
}

fn default_output_dir() -> String {
    "/tmp/corely_collection".to_string()
}

impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            screen: ScreenConfig::default(),
            camera: CameraConfig::default(),
            audio_input: AudioInputConfig::default(),
            audio_output: AudioOutputConfig::default(),
            input_logging: InputLoggingConfig::default(),
            directory_sync: DirectorySyncConfig::default(),
            chunk_duration_secs: default_chunk_duration(),
            output_dir: default_output_dir(),
        }
    }
}

impl CollectionConfig {
    /// Check if any data collection feature is enabled
    pub fn any_enabled(&self) -> bool {
        self.screen.enabled
            || self.camera.enabled
            || self.audio_input.enabled
            || self.audio_output.enabled
            || self.input_logging.enabled
            || !self.directory_sync.paths.is_empty()
    }
}

/// Screen capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    /// Enable screen capture
    #[serde(default)]
    pub enabled: bool,

    /// Frames per second (default: 1)
    #[serde(default = "default_screen_fps")]
    pub fps: u32,

    /// Target resolution height (default: 720)
    #[serde(default = "default_resolution")]
    pub resolution: u32,

    /// Capture all displays (default: true)
    #[serde(default = "default_true")]
    pub all_displays: bool,

    /// Specific display IDs to capture (if all_displays is false)
    #[serde(default)]
    pub display_ids: Vec<u32>,

    /// Quality (0-100, default: 80)
    #[serde(default = "default_quality")]
    pub quality: u32,
}

fn default_screen_fps() -> u32 {
    1
}

fn default_resolution() -> u32 {
    720
}

fn default_true() -> bool {
    true
}

fn default_quality() -> u32 {
    80
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fps: default_screen_fps(),
            resolution: default_resolution(),
            all_displays: true,
            display_ids: Vec::new(),
            quality: default_quality(),
        }
    }
}

/// Camera capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Enable camera capture
    #[serde(default)]
    pub enabled: bool,

    /// Frames per second (default: 5)
    #[serde(default = "default_camera_fps")]
    pub fps: u32,

    /// Target resolution height (default: 480)
    #[serde(default = "default_camera_resolution")]
    pub resolution: u32,

    /// Capture all cameras (default: true)
    #[serde(default = "default_true")]
    pub all_cameras: bool,

    /// Specific camera indices to capture
    #[serde(default)]
    pub camera_indices: Vec<u32>,

    /// Automatically enable microphone when camera is enabled
    #[serde(default = "default_true")]
    pub implies_mic: bool,
}

fn default_camera_fps() -> u32 {
    5
}

fn default_camera_resolution() -> u32 {
    480
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fps: default_camera_fps(),
            resolution: default_camera_resolution(),
            all_cameras: true,
            camera_indices: Vec::new(),
            implies_mic: true,
        }
    }
}

/// Audio input (microphone) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInputConfig {
    /// Enable microphone capture
    #[serde(default)]
    pub enabled: bool,

    /// Sample rate (default: 44100)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// Specific device name (optional)
    pub device: Option<String>,
}

fn default_sample_rate() -> u32 {
    44100
}

impl Default for AudioInputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_rate: default_sample_rate(),
            device: None,
        }
    }
}

/// Audio output (system loopback) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutputConfig {
    /// Enable system audio loopback capture
    #[serde(default)]
    pub enabled: bool,

    /// Sample rate (default: 44100)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// Specific device name (optional)
    pub device: Option<String>,
}

impl Default for AudioOutputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_rate: default_sample_rate(),
            device: None,
        }
    }
}

/// Input logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputLoggingConfig {
    /// Enable input logging
    #[serde(default)]
    pub enabled: bool,

    /// Log keystrokes
    #[serde(default = "default_true")]
    pub log_keystrokes: bool,

    /// Log mouse movements
    #[serde(default = "default_true")]
    pub log_mouse_moves: bool,

    /// Log mouse clicks
    #[serde(default = "default_true")]
    pub log_mouse_clicks: bool,

    /// Mouse movement sampling interval in ms (default: 100)
    #[serde(default = "default_mouse_sample_ms")]
    pub mouse_sample_ms: u64,
}

fn default_mouse_sample_ms() -> u64 {
    100
}

impl Default for InputLoggingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_keystrokes: true,
            log_mouse_moves: true,
            log_mouse_clicks: true,
            mouse_sample_ms: default_mouse_sample_ms(),
        }
    }
}

/// Directory sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorySyncConfig {
    /// Paths to sync
    #[serde(default)]
    pub paths: Vec<String>,

    /// File patterns to include (glob patterns)
    #[serde(default)]
    pub include_patterns: Vec<String>,

    /// File patterns to exclude (glob patterns)
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Sync interval in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,

    /// Maximum file size in bytes (default: 100MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Watch for real-time changes
    #[serde(default = "default_true")]
    pub watch_changes: bool,
}

fn default_sync_interval() -> u64 {
    300
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100MB
}

impl Default for DirectorySyncConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            sync_interval_secs: default_sync_interval(),
            max_file_size: default_max_file_size(),
            watch_changes: true,
        }
    }
}

/// Current collection status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStatus {
    /// Is collection currently in progress
    pub is_collecting: bool,

    /// Current session ID
    pub session_id: Option<String>,

    /// When collection started
    pub started_at: Option<String>,

    /// When collection ended
    pub ended_at: Option<String>,

    /// Number of chunks collected
    pub chunk_count: u64,

    /// Active stream types
    pub active_streams: Vec<StreamType>,

    /// Last error if any
    pub last_error: Option<String>,
}

/// Types of data streams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamType {
    Screen,
    Camera,
    AudioInput,
    AudioOutput,
    InputEvents,
    DirectorySync,
}

/// Multi-timestamp for synchronization (from loggy3 pattern)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MultiTimestamp {
    /// Sequence number within the chunk
    pub seq: u64,
    /// Wall clock time (UTC milliseconds)
    pub wall_time_ms: i64,
    /// Monotonic time (nanoseconds from an arbitrary start)
    pub monotonic_ns: u64,
}

impl MultiTimestamp {
    pub fn now(seq: u64) -> Self {
        use std::time::Instant;

        // Use a lazy static for monotonic base
        lazy_static::lazy_static! {
            static ref MONOTONIC_BASE: Instant = Instant::now();
        }

        Self {
            seq,
            wall_time_ms: chrono::Utc::now().timestamp_millis(),
            monotonic_ns: MONOTONIC_BASE.elapsed().as_nanos() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_config_default() {
        let config = CollectionConfig::default();

        assert!(!config.screen.enabled);
        assert!(!config.camera.enabled);
        assert!(!config.audio_input.enabled);
        assert!(!config.audio_output.enabled);
        assert!(!config.input_logging.enabled);
        assert_eq!(config.chunk_duration_secs, 60);
        assert_eq!(config.output_dir, "/tmp/corely_collection");
    }

    #[test]
    fn test_screen_config_default() {
        let config = ScreenConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.fps, 1);
        assert_eq!(config.resolution, 720);
        assert!(config.all_displays);
        assert!(config.display_ids.is_empty());
        assert_eq!(config.quality, 80);
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.fps, 5);
        assert_eq!(config.resolution, 480);
        assert!(config.all_cameras);
        assert!(config.implies_mic);
    }

    #[test]
    fn test_audio_input_config_default() {
        let config = AudioInputConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.sample_rate, 44100);
        assert!(config.device.is_none());
    }

    #[test]
    fn test_audio_output_config_default() {
        let config = AudioOutputConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.sample_rate, 44100);
        assert!(config.device.is_none());
    }

    #[test]
    fn test_input_logging_config_default() {
        let config = InputLoggingConfig::default();

        assert!(!config.enabled);
        assert!(config.log_keystrokes);
        assert!(config.log_mouse_moves);
        assert!(config.log_mouse_clicks);
        assert_eq!(config.mouse_sample_ms, 100);
    }

    #[test]
    fn test_directory_sync_config_default() {
        let config = DirectorySyncConfig::default();

        assert!(config.paths.is_empty());
        assert!(config.include_patterns.is_empty());
        assert!(config.exclude_patterns.is_empty());
        assert_eq!(config.sync_interval_secs, 300);
        assert_eq!(config.max_file_size, 100 * 1024 * 1024);
        assert!(config.watch_changes);
    }

    #[test]
    fn test_collection_status_default() {
        let status = CollectionStatus::default();

        assert!(!status.is_collecting);
        assert!(status.session_id.is_none());
        assert!(status.started_at.is_none());
        assert!(status.ended_at.is_none());
        assert_eq!(status.chunk_count, 0);
        assert!(status.active_streams.is_empty());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn test_multi_timestamp_now() {
        let ts1 = MultiTimestamp::now(0);
        let ts2 = MultiTimestamp::now(1);

        assert_eq!(ts1.seq, 0);
        assert_eq!(ts2.seq, 1);
        assert!(ts1.wall_time_ms > 0);
        assert!(ts2.wall_time_ms >= ts1.wall_time_ms);
        assert!(ts2.monotonic_ns >= ts1.monotonic_ns);
    }

    #[test]
    fn test_multi_timestamp_sequence() {
        let mut timestamps = Vec::new();
        for i in 0..100 {
            timestamps.push(MultiTimestamp::now(i));
        }

        for (i, ts) in timestamps.iter().enumerate() {
            assert_eq!(ts.seq, i as u64);
        }

        // Monotonic time should be non-decreasing
        for i in 1..timestamps.len() {
            assert!(timestamps[i].monotonic_ns >= timestamps[i-1].monotonic_ns);
        }
    }

    #[test]
    fn test_stream_type_serialization() {
        let types = vec![
            StreamType::Screen,
            StreamType::Camera,
            StreamType::AudioInput,
            StreamType::AudioOutput,
            StreamType::InputEvents,
            StreamType::DirectorySync,
        ];

        for stream_type in types {
            let json = serde_json::to_string(&stream_type).unwrap();
            let deserialized: StreamType = serde_json::from_str(&json).unwrap();
            assert_eq!(stream_type, deserialized);
        }
    }

    #[test]
    fn test_collection_config_serialization() {
        let config = CollectionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CollectionConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.screen.enabled, deserialized.screen.enabled);
        assert_eq!(config.screen.fps, deserialized.screen.fps);
        assert_eq!(config.chunk_duration_secs, deserialized.chunk_duration_secs);
    }

    #[test]
    fn test_collection_config_with_custom_values() {
        let config = CollectionConfig {
            screen: ScreenConfig {
                enabled: true,
                fps: 5,
                resolution: 1080,
                all_displays: false,
                display_ids: vec![0, 1],
                quality: 90,
            },
            camera: CameraConfig {
                enabled: true,
                fps: 10,
                resolution: 720,
                all_cameras: false,
                camera_indices: vec![0],
                implies_mic: false,
            },
            audio_input: AudioInputConfig {
                enabled: true,
                sample_rate: 48000,
                device: Some("Microphone".to_string()),
            },
            audio_output: AudioOutputConfig {
                enabled: true,
                sample_rate: 48000,
                device: None,
            },
            input_logging: InputLoggingConfig {
                enabled: true,
                log_keystrokes: true,
                log_mouse_moves: false,
                log_mouse_clicks: true,
                mouse_sample_ms: 50,
            },
            directory_sync: DirectorySyncConfig {
                paths: vec!["/home/user/docs".to_string()],
                include_patterns: vec!["*.txt".to_string()],
                exclude_patterns: vec!["*.tmp".to_string()],
                sync_interval_secs: 600,
                max_file_size: 50 * 1024 * 1024,
                watch_changes: true,
            },
            chunk_duration_secs: 120,
            output_dir: "/custom/path".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CollectionConfig = serde_json::from_str(&json).unwrap();

        assert!(deserialized.screen.enabled);
        assert_eq!(deserialized.screen.fps, 5);
        assert_eq!(deserialized.screen.resolution, 1080);
        assert!(!deserialized.screen.all_displays);
        assert_eq!(deserialized.screen.display_ids, vec![0, 1]);

        assert!(deserialized.camera.enabled);
        assert!(!deserialized.camera.implies_mic);

        assert_eq!(deserialized.audio_input.sample_rate, 48000);
        assert_eq!(deserialized.audio_input.device, Some("Microphone".to_string()));

        assert_eq!(deserialized.directory_sync.paths, vec!["/home/user/docs".to_string()]);
        assert_eq!(deserialized.chunk_duration_secs, 120);
    }

    #[test]
    fn test_collection_status_serialization() {
        let status = CollectionStatus {
            is_collecting: true,
            session_id: Some("test-session-123".to_string()),
            started_at: Some("2024-01-15T10:30:00Z".to_string()),
            ended_at: None,
            chunk_count: 5,
            active_streams: vec![StreamType::Screen, StreamType::AudioInput],
            last_error: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: CollectionStatus = serde_json::from_str(&json).unwrap();

        assert!(deserialized.is_collecting);
        assert_eq!(deserialized.session_id, Some("test-session-123".to_string()));
        assert_eq!(deserialized.chunk_count, 5);
        assert_eq!(deserialized.active_streams.len(), 2);
    }

    #[test]
    fn test_multi_timestamp_serialization() {
        let ts = MultiTimestamp {
            seq: 42,
            wall_time_ms: 1705312200000,
            monotonic_ns: 123456789,
        };

        let json = serde_json::to_string(&ts).unwrap();
        let deserialized: MultiTimestamp = serde_json::from_str(&json).unwrap();

        assert_eq!(ts.seq, deserialized.seq);
        assert_eq!(ts.wall_time_ms, deserialized.wall_time_ms);
        assert_eq!(ts.monotonic_ns, deserialized.monotonic_ns);
    }
}
