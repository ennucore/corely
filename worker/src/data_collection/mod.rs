//! Data collection module for streaming screen capture, audio, camera, input, and directory sync.
//!
//! This module orchestrates all data collection capabilities:
//! - Screen capture (all displays, configurable FPS)
//! - Camera capture (all cameras)
//! - Audio capture (microphone + system output loopback)
//! - Input logging (keystrokes and mouse events)
//! - Directory sync (file backup with hashing)
//!
//! Data is organized into 60-second chunks with synchronized timestamps.

pub mod config;
pub mod chunk_manager;
pub mod screen_stream;
pub mod audio_capture;
pub mod input_logger;
pub mod camera_stream;
pub mod directory_sync;
pub mod encoder;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, broadcast};
use tracing::{debug, error, info, warn};

use config::{CollectionConfig, CollectionStatus, StreamType};
use chunk_manager::ChunkManager;

/// Orchestrates all data collection activities
pub struct CollectionOrchestrator {
    config: Arc<RwLock<CollectionConfig>>,
    status: Arc<RwLock<CollectionStatus>>,
    chunk_manager: Arc<Mutex<Option<ChunkManager>>>,
    shutdown_tx: Arc<Mutex<Option<broadcast::Sender<()>>>>,
    worker_id: String,
}

impl CollectionOrchestrator {
    pub fn new(worker_id: String) -> Self {
        Self {
            config: Arc::new(RwLock::new(CollectionConfig::default())),
            status: Arc::new(RwLock::new(CollectionStatus::default())),
            chunk_manager: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            worker_id,
        }
    }

    /// Update collection configuration
    pub async fn update_config(&self, new_config: CollectionConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Collection config updated");
        Ok(())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> CollectionConfig {
        self.config.read().await.clone()
    }

    /// Get current status
    pub async fn get_status(&self) -> CollectionStatus {
        self.status.read().await.clone()
    }

    /// Start data collection based on current config
    pub async fn start(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        let mut status = self.status.write().await;

        if status.is_collecting {
            return Err(anyhow!("Collection already in progress"));
        }

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx.clone());

        // Create chunk manager
        let session_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let chunk_manager = ChunkManager::new(
            &self.worker_id,
            &session_id,
            config.chunk_duration_secs,
        )?;
        *self.chunk_manager.lock().await = Some(chunk_manager);

        // Update status
        status.is_collecting = true;
        status.session_id = Some(session_id.clone());
        status.started_at = Some(chrono::Utc::now().to_rfc3339());

        // Spawn collection tasks based on config
        let orchestrator = self.clone_refs();
        let config_clone = config.clone();

        tokio::spawn(async move {
            if let Err(e) = run_collection_loop(orchestrator, config_clone).await {
                error!("Collection loop error: {}", e);
            }
        });

        info!("Data collection started, session: {}", session_id);
        Ok(())
    }

    /// Stop data collection
    pub async fn stop(&self) -> Result<()> {
        let mut status = self.status.write().await;

        if !status.is_collecting {
            return Err(anyhow!("Collection not in progress"));
        }

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }

        // Finalize chunk manager
        if let Some(mut cm) = self.chunk_manager.lock().await.take() {
            cm.finalize().await?;
        }

        // Update status
        status.is_collecting = false;
        status.ended_at = Some(chrono::Utc::now().to_rfc3339());

        info!("Data collection stopped");
        Ok(())
    }

    fn clone_refs(&self) -> OrchestratorRefs {
        OrchestratorRefs {
            config: self.config.clone(),
            status: self.status.clone(),
            chunk_manager: self.chunk_manager.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }

}

async fn run_collection_loop(orchestrator: OrchestratorRefs, config: CollectionConfig) -> Result<()> {
    let mut shutdown_rx = orchestrator.get_shutdown_receiver().await;

    // Start screen capture if enabled
    if config.screen.enabled {
        let orch = orchestrator.clone();
        let screen_config = config.screen.clone();
        tokio::spawn(async move {
            if let Err(e) = screen_stream::run_capture(orch, screen_config).await {
                error!("Screen capture error: {}", e);
            }
        });
    }

    // Start audio capture if enabled
    if config.audio_input.enabled || config.audio_output.enabled {
        let orch = orchestrator.clone();
        let input_enabled = config.audio_input.enabled;
        let output_enabled = config.audio_output.enabled;
        tokio::spawn(async move {
            if let Err(e) = audio_capture::run_capture(orch, input_enabled, output_enabled).await {
                error!("Audio capture error: {}", e);
            }
        });
    }

    // Start camera capture if enabled
    if config.camera.enabled {
        let orch = orchestrator.clone();
        let camera_config = config.camera.clone();
        tokio::spawn(async move {
            if let Err(e) = camera_stream::run_capture(orch, camera_config).await {
                error!("Camera capture error: {}", e);
            }
        });
    }

    // Start input logging if enabled
    if config.input_logging.enabled {
        let orch = orchestrator.clone();
        tokio::spawn(async move {
            if let Err(e) = input_logger::run_capture(orch).await {
                error!("Input logging error: {}", e);
            }
        });
    }

    // Start directory sync if enabled
    if !config.directory_sync.paths.is_empty() {
        let orch = orchestrator.clone();
        let sync_config = config.directory_sync.clone();
        tokio::spawn(async move {
            if let Err(e) = directory_sync::run_sync(orch, sync_config).await {
                error!("Directory sync error: {}", e);
            }
        });
    }

    // Wait for shutdown signal
    if let Some(ref mut rx) = shutdown_rx {
        let _ = rx.recv().await;
    }

    Ok(())
}

/// Shared references for spawned tasks
#[derive(Clone)]
pub struct OrchestratorRefs {
    pub config: Arc<RwLock<CollectionConfig>>,
    pub status: Arc<RwLock<CollectionStatus>>,
    pub chunk_manager: Arc<Mutex<Option<ChunkManager>>>,
    pub shutdown_tx: Arc<Mutex<Option<broadcast::Sender<()>>>>,
}

impl OrchestratorRefs {
    pub async fn should_stop(&self) -> bool {
        let tx = self.shutdown_tx.lock().await;
        tx.is_none()
    }

    pub async fn get_shutdown_receiver(&self) -> Option<broadcast::Receiver<()>> {
        let tx = self.shutdown_tx.lock().await;
        tx.as_ref().map(|t| t.subscribe())
    }
}

/// JSON-RPC handlers for collection commands
pub async fn handle_update_config(params: Value, orchestrator: &CollectionOrchestrator) -> Result<Value> {
    let config: CollectionConfig = serde_json::from_value(params)?;
    let should_collect = config.any_enabled();
    let current_status = orchestrator.get_status().await;

    // Update the config first
    orchestrator.update_config(config).await?;

    // Auto-start/stop based on config
    if should_collect && !current_status.is_collecting {
        tracing::info!("Config has collection enabled - starting collection automatically");
        orchestrator.start().await?;
    } else if !should_collect && current_status.is_collecting {
        tracing::info!("Config has no collection enabled - stopping collection automatically");
        orchestrator.stop().await?;
    }

    let new_status = orchestrator.get_status().await;
    Ok(json!({
        "status": "ok",
        "is_collecting": new_status.is_collecting,
        "session_id": new_status.session_id,
    }))
}

pub async fn handle_start(orchestrator: &CollectionOrchestrator) -> Result<Value> {
    orchestrator.start().await?;
    let status = orchestrator.get_status().await;
    Ok(json!({
        "status": "started",
        "session_id": status.session_id,
    }))
}

pub async fn handle_stop(orchestrator: &CollectionOrchestrator) -> Result<Value> {
    orchestrator.stop().await?;
    Ok(json!({"status": "stopped"}))
}

pub async fn handle_status(orchestrator: &CollectionOrchestrator) -> Result<Value> {
    let status = orchestrator.get_status().await;
    let config = orchestrator.get_config().await;
    Ok(json!({
        "is_collecting": status.is_collecting,
        "session_id": status.session_id,
        "started_at": status.started_at,
        "ended_at": status.ended_at,
        "chunk_count": status.chunk_count,
        "config": config,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::*;

    #[test]
    fn test_orchestrator_new() {
        let orchestrator = CollectionOrchestrator::new("test-worker".to_string());
        assert_eq!(orchestrator.worker_id, "test-worker");
    }

    #[tokio::test]
    async fn test_orchestrator_default_config() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());
        let config = orchestrator.get_config().await;

        assert!(!config.screen.enabled);
        assert!(!config.camera.enabled);
        assert!(!config.audio_input.enabled);
        assert!(!config.audio_output.enabled);
        assert!(!config.input_logging.enabled);
    }

    #[tokio::test]
    async fn test_orchestrator_default_status() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());
        let status = orchestrator.get_status().await;

        assert!(!status.is_collecting);
        assert!(status.session_id.is_none());
        assert!(status.started_at.is_none());
        assert!(status.ended_at.is_none());
        assert_eq!(status.chunk_count, 0);
    }

    #[tokio::test]
    async fn test_orchestrator_update_config() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());

        let new_config = CollectionConfig {
            screen: ScreenConfig {
                enabled: true,
                fps: 5,
                resolution: 720,
                all_displays: true,
                display_ids: vec![],
                quality: 80,
            },
            ..Default::default()
        };

        orchestrator.update_config(new_config).await.unwrap();

        let config = orchestrator.get_config().await;
        assert!(config.screen.enabled);
        assert_eq!(config.screen.fps, 5);
    }

    #[tokio::test]
    async fn test_orchestrator_cannot_stop_when_not_collecting() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());

        let result = orchestrator.stop().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_orchestrator_refs_clone() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());
        let refs = orchestrator.clone_refs();

        // Test that refs are cloneable
        let refs2 = refs.clone();
        // Initially, there's no shutdown channel, so should_stop returns true
        assert!(refs2.should_stop().await);
    }

    #[tokio::test]
    async fn test_orchestrator_refs_should_stop() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());
        let refs = orchestrator.clone_refs();

        // Initially no shutdown channel, so should_stop is true
        assert!(refs.should_stop().await);
    }

    #[tokio::test]
    async fn test_orchestrator_refs_get_shutdown_receiver() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());
        let refs = orchestrator.clone_refs();

        // Initially no shutdown channel
        let rx = refs.get_shutdown_receiver().await;
        assert!(rx.is_none());
    }

    #[tokio::test]
    async fn test_handle_update_config_valid() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());

        let params = json!({
            "screen": {
                "enabled": true,
                "fps": 10,
                "resolution": 1080,
                "all_displays": true,
                "display_ids": [],
                "quality": 90
            },
            "camera": {
                "enabled": false,
                "fps": 5,
                "resolution": 720,
                "all_cameras": true,
                "camera_indices": []
            },
            "audio_input": {
                "enabled": false,
                "sample_rate": 44100,
                "device": null
            },
            "audio_output": {
                "enabled": false,
                "sample_rate": 44100,
                "device": null
            },
            "input_logging": {
                "enabled": false,
                "log_keys": true,
                "log_mouse_clicks": true,
                "log_mouse_movement": false
            },
            "directory_sync": {
                "paths": [],
                "include_patterns": [],
                "exclude_patterns": [],
                "sync_interval_secs": 300,
                "max_file_size": 104857600,
                "watch_changes": false
            },
            "chunk_duration_secs": 60
        });

        let result = handle_update_config(params, &orchestrator).await;
        assert!(result.is_ok());

        let config = orchestrator.get_config().await;
        assert!(config.screen.enabled);
        assert_eq!(config.screen.fps, 10);
    }

    #[tokio::test]
    async fn test_handle_status_not_collecting() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());

        let result = handle_status(&orchestrator).await.unwrap();

        assert_eq!(result["is_collecting"], false);
        assert!(result["session_id"].is_null());
    }

    #[test]
    fn test_collection_config_serialization() {
        let config = CollectionConfig::default();
        let json = serde_json::to_value(&config).unwrap();

        assert!(json.get("screen").is_some());
        assert!(json.get("camera").is_some());
        assert!(json.get("audio_input").is_some());
        assert!(json.get("audio_output").is_some());
        assert!(json.get("input_logging").is_some());
        assert!(json.get("directory_sync").is_some());
    }

    #[test]
    fn test_collection_config_deserialization() {
        let json = r#"{
            "screen": {
                "enabled": true,
                "fps": 5,
                "resolution": 720,
                "all_displays": true,
                "display_ids": [],
                "quality": 80
            },
            "camera": {
                "enabled": false,
                "fps": 5,
                "resolution": 720,
                "all_cameras": true,
                "camera_indices": []
            },
            "audio_input": {
                "enabled": true,
                "sample_rate": 44100,
                "device": null
            },
            "audio_output": {
                "enabled": true,
                "sample_rate": 48000,
                "device": null
            },
            "input_logging": {
                "enabled": false,
                "log_keys": true,
                "log_mouse_clicks": true,
                "log_mouse_movement": false
            },
            "directory_sync": {
                "paths": ["/home/user"],
                "include_patterns": ["*.txt"],
                "exclude_patterns": [],
                "sync_interval_secs": 300,
                "max_file_size": 104857600,
                "watch_changes": true
            },
            "chunk_duration_secs": 60
        }"#;

        let config: CollectionConfig = serde_json::from_str(json).unwrap();
        assert!(config.screen.enabled);
        assert!(config.audio_input.enabled);
        assert_eq!(config.audio_output.sample_rate, 48000);
        assert_eq!(config.directory_sync.paths, vec!["/home/user"]);
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
    }

    #[test]
    fn test_collection_status_clone() {
        let mut status = CollectionStatus::default();
        status.is_collecting = true;
        status.session_id = Some("test_session".to_string());

        let cloned = status.clone();
        assert!(cloned.is_collecting);
        assert_eq!(cloned.session_id, Some("test_session".to_string()));
    }

    #[test]
    fn test_stream_type_variants() {
        let types = vec![
            StreamType::Screen,
            StreamType::Camera,
            StreamType::AudioInput,
            StreamType::AudioOutput,
            StreamType::InputEvents,
            StreamType::DirectorySync,
        ];

        assert_eq!(types.len(), 6);

        // Test clone
        for t in &types {
            let cloned = t.clone();
            assert!(format!("{:?}", cloned).len() > 0);
        }
    }

    #[test]
    fn test_stream_type_serialization() {
        let stream_type = StreamType::Screen;
        let json = serde_json::to_string(&stream_type).unwrap();
        assert!(json.contains("Screen"));
    }

    #[tokio::test]
    async fn test_concurrent_config_access() {
        let orchestrator = Arc::new(CollectionOrchestrator::new("test".to_string()));

        let orchestrator1 = orchestrator.clone();
        let orchestrator2 = orchestrator.clone();

        let handle1 = tokio::spawn(async move {
            for _ in 0..10 {
                let _ = orchestrator1.get_config().await;
            }
        });

        let handle2 = tokio::spawn(async move {
            for _ in 0..10 {
                let _ = orchestrator2.get_status().await;
            }
        });

        handle1.await.unwrap();
        handle2.await.unwrap();
    }

    #[tokio::test]
    async fn test_orchestrator_multiple_config_updates() {
        let orchestrator = CollectionOrchestrator::new("test".to_string());

        for i in 1..=5 {
            let new_config = CollectionConfig {
                screen: ScreenConfig {
                    enabled: true,
                    fps: i,
                    resolution: 720,
                    all_displays: true,
                    display_ids: vec![],
                    quality: 80,
                },
                ..Default::default()
            };

            orchestrator.update_config(new_config).await.unwrap();

            let config = orchestrator.get_config().await;
            assert_eq!(config.screen.fps, i);
        }
    }

    #[test]
    fn test_session_id_format() {
        let session_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();

        // Should be in format YYYYMMDD_HHMMSS (15 chars total)
        assert_eq!(session_id.len(), 15);
        assert!(session_id.contains('_'));

        // First 8 chars should be date
        let date_part: String = session_id.chars().take(8).collect();
        assert!(date_part.parse::<u32>().is_ok());
    }

    #[test]
    fn test_json_response_start() {
        let session_id = "20240115_120000";
        let response = json!({
            "status": "started",
            "session_id": session_id,
        });

        assert_eq!(response["status"], "started");
        assert_eq!(response["session_id"], session_id);
    }

    #[test]
    fn test_json_response_stop() {
        let response = json!({"status": "stopped"});
        assert_eq!(response["status"], "stopped");
    }

    #[test]
    fn test_json_response_ok() {
        let response = json!({"status": "ok"});
        assert_eq!(response["status"], "ok");
    }

    #[tokio::test]
    async fn test_broadcast_channel_creation() {
        let (tx, mut rx) = broadcast::channel(1);

        // Send signal
        let _ = tx.send(());

        // Receive signal
        let result = rx.recv().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_channel_multiple_receivers() {
        let (tx, mut rx1) = broadcast::channel(1);
        let mut rx2 = tx.subscribe();

        let _ = tx.send(());

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_chunk_duration_default() {
        let config = CollectionConfig::default();
        assert_eq!(config.chunk_duration_secs, 60);
    }

    #[test]
    fn test_chunk_duration_custom() {
        let config = CollectionConfig {
            chunk_duration_secs: 120,
            ..Default::default()
        };
        assert_eq!(config.chunk_duration_secs, 120);
    }
}
