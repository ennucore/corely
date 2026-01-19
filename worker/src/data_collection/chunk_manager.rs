//! Chunk manager for organizing collected data into 60-second segments.
//!
//! Chunk structure:
//! ```
//! session_YYYYMMDD_HHMMSS/
//! ├── chunk_00000/
//! │   ├── display_0/video.mp4
//! │   ├── display_1/video.mp4
//! │   ├── camera_0/video.mp4
//! │   ├── mic_audio.pcm
//! │   ├── output_audio.pcm
//! │   ├── input.log
//! │   └── frames.log
//! ├── chunk_00001/
//! │   └── ...
//! └── session_metadata.json
//! ```

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::config::MultiTimestamp;

/// Manages chunked data collection
pub struct ChunkManager {
    worker_id: String,
    session_id: String,
    base_dir: PathBuf,
    chunk_duration: Duration,
    current_chunk: AtomicU64,
    chunk_start: Mutex<Instant>,
    writers: Mutex<HashMap<String, ChunkWriter>>,
    metadata: Mutex<SessionMetadata>,
}

/// Metadata for a collection session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub worker_id: String,
    pub session_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub chunk_count: u64,
    pub streams: Vec<String>,
}

/// Writer for a specific stream within a chunk
pub struct ChunkWriter {
    stream_name: String,
    file: BufWriter<File>,
    frame_count: u64,
    bytes_written: u64,
}

impl ChunkWriter {
    pub fn new(path: &Path, stream_name: &str) -> Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            stream_name: stream_name.to_string(),
            file: BufWriter::new(file),
            frame_count: 0,
            bytes_written: 0,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.file.write_all(data)?;
        self.frame_count += 1;
        self.bytes_written += data.len() as u64;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.file.flush()?;
        Ok(())
    }
}

impl ChunkManager {
    pub fn new(worker_id: &str, session_id: &str, chunk_duration_secs: u64) -> Result<Self> {
        let base_dir = PathBuf::from("/tmp/corely_collection")
            .join(format!("session_{}", session_id));

        fs::create_dir_all(&base_dir)?;

        let metadata = SessionMetadata {
            worker_id: worker_id.to_string(),
            session_id: session_id.to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            ended_at: None,
            chunk_count: 0,
            streams: Vec::new(),
        };

        // Write initial metadata
        let metadata_path = base_dir.join("session_metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, metadata_json)?;

        Ok(Self {
            worker_id: worker_id.to_string(),
            session_id: session_id.to_string(),
            base_dir,
            chunk_duration: Duration::from_secs(chunk_duration_secs),
            current_chunk: AtomicU64::new(0),
            chunk_start: Mutex::new(Instant::now()),
            writers: Mutex::new(HashMap::new()),
            metadata: Mutex::new(metadata),
        })
    }

    /// Get the current chunk index
    pub fn current_chunk_index(&self) -> u64 {
        self.current_chunk.load(Ordering::SeqCst)
    }

    /// Check if we need to rotate to a new chunk
    pub async fn should_rotate(&self) -> bool {
        let start = self.chunk_start.lock().await;
        start.elapsed() >= self.chunk_duration
    }

    /// Rotate to a new chunk
    pub async fn rotate(&self) -> Result<u64> {
        // Flush and close all current writers
        let mut writers = self.writers.lock().await;
        for (_, writer) in writers.iter_mut() {
            writer.flush()?;
        }
        writers.clear();

        // Update metadata
        let mut metadata = self.metadata.lock().await;
        metadata.chunk_count += 1;

        // Increment chunk counter
        let new_chunk = self.current_chunk.fetch_add(1, Ordering::SeqCst) + 1;

        // Reset chunk start time
        let mut start = self.chunk_start.lock().await;
        *start = Instant::now();

        info!("Rotated to chunk {}", new_chunk);
        Ok(new_chunk)
    }

    /// Get or create a writer for a stream
    pub async fn get_writer(&self, stream_name: &str) -> Result<()> {
        let chunk_idx = self.current_chunk.load(Ordering::SeqCst);
        let chunk_dir = self.base_dir.join(format!("chunk_{:05}", chunk_idx));
        fs::create_dir_all(&chunk_dir)?;

        let mut writers = self.writers.lock().await;
        if !writers.contains_key(stream_name) {
            let file_path = chunk_dir.join(stream_name);

            // Create parent directories if needed (for display_0/video.mp4 etc)
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let writer = ChunkWriter::new(&file_path, stream_name)?;
            writers.insert(stream_name.to_string(), writer);

            // Update metadata with stream type
            let mut metadata = self.metadata.lock().await;
            if !metadata.streams.contains(&stream_name.to_string()) {
                metadata.streams.push(stream_name.to_string());
            }
        }
        Ok(())
    }

    /// Write data to a stream
    pub async fn write_data(&self, stream_name: &str, data: &[u8]) -> Result<()> {
        // Check for rotation
        if self.should_rotate().await {
            self.rotate().await?;
        }

        // Ensure writer exists
        self.get_writer(stream_name).await?;

        let mut writers = self.writers.lock().await;
        if let Some(writer) = writers.get_mut(stream_name) {
            writer.write(data)?;
        }
        Ok(())
    }

    /// Write a frame with timestamp to a stream
    pub async fn write_frame(&self, stream_name: &str, data: &[u8], timestamp: MultiTimestamp) -> Result<()> {
        // Check for rotation
        if self.should_rotate().await {
            self.rotate().await?;
        }

        // Ensure writer exists
        self.get_writer(stream_name).await?;

        let mut writers = self.writers.lock().await;

        // Write data to main stream
        if let Some(writer) = writers.get_mut(stream_name) {
            writer.write(data)?;
        }

        // Write timestamp to frames.log
        let frames_log = format!("{}_frames.log", stream_name.replace("/", "_"));
        if !writers.contains_key(&frames_log) {
            let chunk_idx = self.current_chunk.load(Ordering::SeqCst);
            let chunk_dir = self.base_dir.join(format!("chunk_{:05}", chunk_idx));
            let file_path = chunk_dir.join(&frames_log);
            let writer = ChunkWriter::new(&file_path, &frames_log)?;
            writers.insert(frames_log.clone(), writer);
        }

        if let Some(log_writer) = writers.get_mut(&frames_log) {
            let log_line = format!(
                "{},{},{}\n",
                timestamp.seq,
                timestamp.wall_time_ms,
                timestamp.monotonic_ns
            );
            log_writer.write(log_line.as_bytes())?;
        }

        Ok(())
    }

    /// Write input events to input.log
    pub async fn write_input_event(&self, event: &serde_json::Value) -> Result<()> {
        if self.should_rotate().await {
            self.rotate().await?;
        }

        self.get_writer("input.log").await?;

        let mut writers = self.writers.lock().await;
        if let Some(writer) = writers.get_mut("input.log") {
            let line = serde_json::to_string(event)? + "\n";
            writer.write(line.as_bytes())?;
        }
        Ok(())
    }

    /// Flush all writers to disk
    pub async fn flush(&self) -> Result<()> {
        let mut writers = self.writers.lock().await;
        for (_, writer) in writers.iter_mut() {
            writer.flush()?;
        }
        Ok(())
    }

    /// Finalize the session
    pub async fn finalize(&mut self) -> Result<()> {
        // Flush all writers
        let mut writers = self.writers.lock().await;
        for (_, writer) in writers.iter_mut() {
            writer.flush()?;
        }
        writers.clear();

        // Update metadata
        let mut metadata = self.metadata.lock().await;
        metadata.ended_at = Some(chrono::Utc::now().to_rfc3339());

        // Write final metadata
        let metadata_path = self.base_dir.join("session_metadata.json");
        let metadata_json = serde_json::to_string_pretty(&*metadata)?;
        fs::write(&metadata_path, metadata_json)?;

        info!("Session finalized: {} chunks", metadata.chunk_count);
        Ok(())
    }

    /// Get the base directory path
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the current chunk directory
    pub fn current_chunk_dir(&self) -> PathBuf {
        let chunk_idx = self.current_chunk.load(Ordering::SeqCst);
        self.base_dir.join(format!("chunk_{:05}", chunk_idx))
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_chunk_manager(session_id: &str, chunk_duration: u64) -> ChunkManager {
        ChunkManager::new("test-worker", session_id, chunk_duration).unwrap()
    }

    #[tokio::test]
    async fn test_chunk_manager_creation() {
        let cm = create_test_chunk_manager("test-session", 60);
        assert_eq!(cm.current_chunk_index(), 0);
        assert_eq!(cm.session_id(), "test-session");
        assert!(cm.base_dir().exists());
    }

    #[tokio::test]
    async fn test_chunk_manager_creates_session_directory() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        assert!(cm.base_dir().exists());
        assert!(cm.base_dir().join("session_metadata.json").exists());

        // Clean up
        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_session_metadata_creation() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        let metadata_path = cm.base_dir().join("session_metadata.json");
        let metadata_content = fs::read_to_string(&metadata_path).unwrap();
        let metadata: SessionMetadata = serde_json::from_str(&metadata_content).unwrap();

        assert_eq!(metadata.worker_id, "test-worker");
        assert_eq!(metadata.session_id, session_id);
        assert!(metadata.ended_at.is_none());
        assert_eq!(metadata.chunk_count, 0);

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_write_data() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        cm.write_data("test.bin", b"hello world").await.unwrap();
        cm.flush().await.unwrap();

        let chunk_dir = cm.current_chunk_dir();
        let file_path = chunk_dir.join("test.bin");
        assert!(file_path.exists());

        let content = fs::read(&file_path).unwrap();
        assert_eq!(content, b"hello world");

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_write_multiple_data_to_same_stream() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        cm.write_data("stream.bin", b"first ").await.unwrap();
        cm.write_data("stream.bin", b"second ").await.unwrap();
        cm.write_data("stream.bin", b"third").await.unwrap();
        cm.flush().await.unwrap();

        let chunk_dir = cm.current_chunk_dir();
        let file_path = chunk_dir.join("stream.bin");

        let content = fs::read(&file_path).unwrap();
        assert_eq!(content, b"first second third");

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_write_frame_with_timestamp() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        let timestamp = MultiTimestamp::now(0);
        cm.write_frame("video.raw", b"frame data", timestamp).await.unwrap();
        cm.flush().await.unwrap();

        let chunk_dir = cm.current_chunk_dir();

        // Check video data
        let video_path = chunk_dir.join("video.raw");
        assert!(video_path.exists());
        let video_content = fs::read(&video_path).unwrap();
        assert_eq!(video_content, b"frame data");

        // Check frames log
        let log_path = chunk_dir.join("video.raw_frames.log");
        assert!(log_path.exists());

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_write_multiple_frames() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        for i in 0..10 {
            let timestamp = MultiTimestamp::now(i);
            let frame_data = format!("frame_{}", i);
            cm.write_frame("video.raw", frame_data.as_bytes(), timestamp).await.unwrap();
        }
        cm.flush().await.unwrap();

        let chunk_dir = cm.current_chunk_dir();
        let log_path = chunk_dir.join("video.raw_frames.log");

        let log_content = fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = log_content.lines().collect();
        assert_eq!(lines.len(), 10);

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_write_input_event() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        let event = serde_json::json!({
            "type": "key_down",
            "key": "a",
            "timestamp": 123456789
        });

        cm.write_input_event(&event).await.unwrap();
        cm.flush().await.unwrap();

        let chunk_dir = cm.current_chunk_dir();
        let log_path = chunk_dir.join("input.log");

        assert!(log_path.exists());

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("key_down"));
        assert!(content.contains("\"a\""));

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_nested_stream_paths() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        cm.write_data("display_0/video.raw", b"display 0 data").await.unwrap();
        cm.write_data("display_1/video.raw", b"display 1 data").await.unwrap();
        cm.write_data("camera_0/video.raw", b"camera data").await.unwrap();

        let chunk_dir = cm.current_chunk_dir();

        assert!(chunk_dir.join("display_0/video.raw").exists());
        assert!(chunk_dir.join("display_1/video.raw").exists());
        assert!(chunk_dir.join("camera_0/video.raw").exists());

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_should_rotate_before_duration() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        // Should not rotate immediately
        assert!(!cm.should_rotate().await);

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_manual_rotation() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        assert_eq!(cm.current_chunk_index(), 0);

        cm.rotate().await.unwrap();
        assert_eq!(cm.current_chunk_index(), 1);

        cm.rotate().await.unwrap();
        assert_eq!(cm.current_chunk_index(), 2);

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_rotation_creates_new_chunk_dir() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        cm.write_data("test.bin", b"chunk 0 data").await.unwrap();
        let chunk_0_dir = cm.current_chunk_dir();

        cm.rotate().await.unwrap();

        cm.write_data("test.bin", b"chunk 1 data").await.unwrap();
        let chunk_1_dir = cm.current_chunk_dir();

        assert_ne!(chunk_0_dir, chunk_1_dir);
        assert!(chunk_0_dir.exists());
        assert!(chunk_1_dir.exists());

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_finalize() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let mut cm = create_test_chunk_manager(&session_id, 60);

        cm.write_data("test.bin", b"some data").await.unwrap();
        cm.finalize().await.unwrap();

        let metadata_path = cm.base_dir().join("session_metadata.json");
        let metadata_content = fs::read_to_string(&metadata_path).unwrap();
        let metadata: SessionMetadata = serde_json::from_str(&metadata_content).unwrap();

        assert!(metadata.ended_at.is_some());

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_chunk_naming_format() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        // Chunk 0
        assert!(cm.current_chunk_dir().ends_with("chunk_00000"));

        cm.rotate().await.unwrap();
        assert!(cm.current_chunk_dir().ends_with("chunk_00001"));

        cm.rotate().await.unwrap();
        assert!(cm.current_chunk_dir().ends_with("chunk_00002"));

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_concurrent_writes() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = std::sync::Arc::new(create_test_chunk_manager(&session_id, 60));

        let mut handles = vec![];

        for i in 0..10 {
            let cm_clone = cm.clone();
            let handle = tokio::spawn(async move {
                let stream = format!("stream_{}.bin", i);
                let data = format!("data from stream {}", i);
                cm_clone.write_data(&stream, data.as_bytes()).await.unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let chunk_dir = cm.current_chunk_dir();
        for i in 0..10 {
            let stream_path = chunk_dir.join(format!("stream_{}.bin", i));
            assert!(stream_path.exists());
        }

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[tokio::test]
    async fn test_large_data_write() {
        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());
        let cm = create_test_chunk_manager(&session_id, 60);

        // Write 1MB of data
        let large_data = vec![0u8; 1024 * 1024];
        cm.write_data("large.bin", &large_data).await.unwrap();

        let chunk_dir = cm.current_chunk_dir();
        let file_path = chunk_dir.join("large.bin");

        let metadata = fs::metadata(&file_path).unwrap();
        assert_eq!(metadata.len(), 1024 * 1024);

        let _ = fs::remove_dir_all(cm.base_dir());
    }

    #[test]
    fn test_session_metadata_serialization() {
        let metadata = SessionMetadata {
            worker_id: "worker-123".to_string(),
            session_id: "session-456".to_string(),
            started_at: "2024-01-15T10:00:00Z".to_string(),
            ended_at: Some("2024-01-15T11:00:00Z".to_string()),
            chunk_count: 60,
            streams: vec!["screen".to_string(), "audio".to_string()],
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: SessionMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.worker_id, deserialized.worker_id);
        assert_eq!(metadata.session_id, deserialized.session_id);
        assert_eq!(metadata.chunk_count, deserialized.chunk_count);
        assert_eq!(metadata.streams, deserialized.streams);
    }

    #[test]
    fn test_chunk_writer_new() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_stream.bin");

        let writer = ChunkWriter::new(&path, "test_stream").unwrap();
        assert_eq!(writer.stream_name, "test_stream");
        assert_eq!(writer.frame_count, 0);
        assert_eq!(writer.bytes_written, 0);
    }

    #[test]
    fn test_chunk_writer_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_stream.bin");

        let mut writer = ChunkWriter::new(&path, "test_stream").unwrap();

        writer.write(b"hello").unwrap();
        assert_eq!(writer.frame_count, 1);
        assert_eq!(writer.bytes_written, 5);

        writer.write(b" world").unwrap();
        assert_eq!(writer.frame_count, 2);
        assert_eq!(writer.bytes_written, 11);

        writer.flush().unwrap();

        let content = fs::read(&path).unwrap();
        assert_eq!(content, b"hello world");
    }

    #[test]
    fn test_chunk_writer_empty_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_stream.bin");

        let mut writer = ChunkWriter::new(&path, "test_stream").unwrap();

        writer.write(b"").unwrap();
        assert_eq!(writer.frame_count, 1);
        assert_eq!(writer.bytes_written, 0);
    }
}
