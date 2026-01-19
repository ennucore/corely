//! Directory synchronization and file backup.
//!
//! Watches configured directories for changes and backs up files
//! with content hashing to detect modifications.

use anyhow::{anyhow, Result};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::config::{DirectorySyncConfig, MultiTimestamp};
use super::OrchestratorRefs;

/// Run directory synchronization
pub async fn run_sync(orchestrator: OrchestratorRefs, config: DirectorySyncConfig) -> Result<()> {
    info!("Starting directory sync for {} paths", config.paths.len());

    let shutdown_rx = orchestrator.get_shutdown_receiver().await;

    // Track file hashes for change detection
    let mut file_hashes: HashMap<PathBuf, String> = HashMap::new();

    // Create file watcher if enabled
    let (tx, rx) = channel();
    let mut watcher: Option<RecommendedWatcher> = None;

    if config.watch_changes {
        match RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        ) {
            Ok(w) => watcher = Some(w),
            Err(e) => warn!("Failed to create file watcher: {}", e),
        }

        // Add paths to watcher
        if let Some(ref mut w) = watcher {
            for path in &config.paths {
                let path = PathBuf::from(path);
                if path.exists() {
                    if let Err(e) = w.watch(&path, RecursiveMode::Recursive) {
                        warn!("Failed to watch {}: {}", path.display(), e);
                    } else {
                        info!("Watching directory: {}", path.display());
                    }
                }
            }
        }
    }

    // Initial sync of all files
    for path in &config.paths {
        let path = PathBuf::from(path);
        if path.exists() {
            sync_directory(
                &orchestrator,
                &path,
                &config,
                &mut file_hashes,
            )
            .await?;
        }
    }

    let sync_interval = Duration::from_secs(config.sync_interval_secs);
    let mut last_sync = Instant::now();

    loop {
        // Check for shutdown
        if let Some(ref _rx) = shutdown_rx {
            // Non-blocking check via orchestrator
            if orchestrator.should_stop().await {
                break;
            }
        }

        // Process file change events
        while let Ok(event) = rx.try_recv() {
            for path in event.paths {
                if should_sync_file(&path, &config) {
                    sync_file(&orchestrator, &path, &config, &mut file_hashes).await?;
                }
            }
        }

        // Periodic full sync
        if last_sync.elapsed() >= sync_interval {
            for path in &config.paths {
                let path = PathBuf::from(path);
                if path.exists() {
                    sync_directory(&orchestrator, &path, &config, &mut file_hashes).await?;
                }
            }
            last_sync = Instant::now();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    info!("Directory sync stopped");
    Ok(())
}

/// Sync all files in a directory
async fn sync_directory(
    orchestrator: &OrchestratorRefs,
    dir: &Path,
    config: &DirectorySyncConfig,
    file_hashes: &mut HashMap<PathBuf, String>,
) -> Result<()> {
    debug!("Syncing directory: {}", dir.display());

    let walker = walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in walker {
        let path = entry.path();
        if should_sync_file(path, config) {
            sync_file(orchestrator, path, config, file_hashes).await?;
        }
    }

    Ok(())
}

/// Check if a file should be synced based on config patterns
fn should_sync_file(path: &Path, config: &DirectorySyncConfig) -> bool {
    let path_str = path.to_string_lossy();

    // Check exclude patterns
    for pattern in &config.exclude_patterns {
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(&path_str) {
                return false;
            }
        }
    }

    // If include patterns are specified, file must match one
    if !config.include_patterns.is_empty() {
        let mut matches_include = false;
        for pattern in &config.include_patterns {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches(&path_str) {
                    matches_include = true;
                    break;
                }
            }
        }
        if !matches_include {
            return false;
        }
    }

    // Check file size
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > config.max_file_size {
            debug!("Skipping large file: {} ({} bytes)", path.display(), metadata.len());
            return false;
        }
    }

    true
}

/// Sync a single file
async fn sync_file(
    orchestrator: &OrchestratorRefs,
    path: &Path,
    config: &DirectorySyncConfig,
    file_hashes: &mut HashMap<PathBuf, String>,
) -> Result<()> {
    // Calculate file hash
    let hash = hash_file(path)?;

    // Check if file has changed
    let path_buf = path.to_path_buf();
    if let Some(old_hash) = file_hashes.get(&path_buf) {
        if *old_hash == hash {
            debug!("File unchanged: {}", path.display());
            return Ok(());
        }
    }

    // File is new or changed, sync it
    info!("Syncing file: {}", path.display());

    // Read file content
    let content = fs::read(path)?;

    // Create file sync event
    let event = serde_json::json!({
        "type": "file_sync",
        "path": path.to_string_lossy(),
        "hash": hash,
        "size": content.len(),
        "timestamp": MultiTimestamp::now(0),
    });

    // Write to chunk manager
    if let Some(ref cm) = *orchestrator.chunk_manager.lock().await {
        // Write event to sync log
        cm.write_input_event(&event).await?;

        // Write file content
        let relative_path = path
            .strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .replace('/', "_");
        let stream_name = format!("files/{}.bin", relative_path);
        let ts = MultiTimestamp::now(0);
        cm.write_frame(&stream_name, &content, ts).await?;
    }

    // Update hash cache
    file_hashes.insert(path_buf, hash);

    Ok(())
}

/// Calculate SHA256 hash of a file
fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hash_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world").unwrap();

        let hash = hash_file(&file_path).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 hex string length
    }

    #[test]
    fn test_hash_file_consistent() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("consistent.txt");
        fs::write(&file_path, "same content").unwrap();

        let hash1 = hash_file(&file_path).unwrap();
        let hash2 = hash_file(&file_path).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_file_different_content() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        fs::write(&file1, "content A").unwrap();
        fs::write(&file2, "content B").unwrap();

        let hash1 = hash_file(&file1).unwrap();
        let hash2 = hash_file(&file2).unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_file_empty() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();

        let hash = hash_file(&file_path).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
        // SHA256 of empty string
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_hash_file_binary() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("binary.bin");
        fs::write(&file_path, &[0u8, 1, 2, 3, 255, 254, 253]).unwrap();

        let hash = hash_file(&file_path).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_file_large() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("large.bin");
        let content = vec![0u8; 100_000]; // 100KB
        fs::write(&file_path, &content).unwrap();

        let hash = hash_file(&file_path).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_hash_file_nonexistent() {
        let result = hash_file(Path::new("/nonexistent/path/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_sync_file() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec!["*.txt".to_string()],
            exclude_patterns: vec!["*.bak".to_string()],
            sync_interval_secs: 300,
            max_file_size: 1024 * 1024,
            watch_changes: true,
        };

        assert!(should_sync_file(Path::new("/foo/bar.txt"), &config));
        assert!(!should_sync_file(Path::new("/foo/bar.bak"), &config));
        assert!(!should_sync_file(Path::new("/foo/bar.log"), &config));
    }

    #[test]
    fn test_should_sync_file_no_patterns() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
            sync_interval_secs: 300,
            max_file_size: 1024 * 1024,
            watch_changes: true,
        };

        // Without patterns, all files should be synced
        assert!(should_sync_file(Path::new("/foo/bar.txt"), &config));
        assert!(should_sync_file(Path::new("/foo/bar.log"), &config));
        assert!(should_sync_file(Path::new("/foo/bar.any"), &config));
    }

    #[test]
    fn test_should_sync_file_exclude_only() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec!["*.tmp".to_string(), "*.bak".to_string()],
            sync_interval_secs: 300,
            max_file_size: 1024 * 1024,
            watch_changes: true,
        };

        assert!(should_sync_file(Path::new("/foo/file.txt"), &config));
        assert!(!should_sync_file(Path::new("/foo/file.tmp"), &config));
        assert!(!should_sync_file(Path::new("/foo/file.bak"), &config));
    }

    #[test]
    fn test_should_sync_file_multiple_include_patterns() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec!["*.txt".to_string(), "*.md".to_string(), "*.rs".to_string()],
            exclude_patterns: vec![],
            sync_interval_secs: 300,
            max_file_size: 1024 * 1024,
            watch_changes: true,
        };

        assert!(should_sync_file(Path::new("/foo/readme.txt"), &config));
        assert!(should_sync_file(Path::new("/foo/readme.md"), &config));
        assert!(should_sync_file(Path::new("/foo/main.rs"), &config));
        assert!(!should_sync_file(Path::new("/foo/main.py"), &config));
    }

    #[test]
    fn test_should_sync_file_exclude_overrides_include() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec!["*.txt".to_string()],
            exclude_patterns: vec!["**/secret*.txt".to_string()],
            sync_interval_secs: 300,
            max_file_size: 1024 * 1024,
            watch_changes: true,
        };

        assert!(should_sync_file(Path::new("/foo/readme.txt"), &config));
        assert!(!should_sync_file(Path::new("/foo/secret_key.txt"), &config));
    }

    #[test]
    fn test_should_sync_file_size_limit() {
        let dir = tempdir().unwrap();
        let small_file = dir.path().join("small.txt");
        let large_file = dir.path().join("large.txt");

        fs::write(&small_file, "small").unwrap();
        fs::write(&large_file, vec![0u8; 10_000]).unwrap();

        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
            sync_interval_secs: 300,
            max_file_size: 100, // 100 bytes max
            watch_changes: true,
        };

        assert!(should_sync_file(&small_file, &config));
        assert!(!should_sync_file(&large_file, &config));
    }

    #[test]
    fn test_directory_sync_config_default() {
        let config = DirectorySyncConfig::default();

        assert!(config.paths.is_empty());
        assert!(config.include_patterns.is_empty());
        assert!(config.exclude_patterns.is_empty());
        assert_eq!(config.sync_interval_secs, 300);
        assert_eq!(config.max_file_size, 100 * 1024 * 1024); // 100MB
        assert!(config.watch_changes);
    }

    #[test]
    fn test_directory_sync_config_custom() {
        let config = DirectorySyncConfig {
            paths: vec!["/home/user/docs".to_string(), "/home/user/code".to_string()],
            include_patterns: vec!["*.txt".to_string()],
            exclude_patterns: vec!["*.tmp".to_string()],
            sync_interval_secs: 600,
            max_file_size: 50 * 1024 * 1024,
            watch_changes: true,
        };

        assert_eq!(config.paths.len(), 2);
        assert_eq!(config.sync_interval_secs, 600);
        assert!(config.watch_changes);
    }

    #[test]
    fn test_file_hash_cache() {
        let mut file_hashes: HashMap<PathBuf, String> = HashMap::new();

        let path = PathBuf::from("/foo/bar.txt");
        let hash = "abc123".to_string();

        file_hashes.insert(path.clone(), hash.clone());

        assert_eq!(file_hashes.get(&path), Some(&hash));
        assert!(file_hashes.contains_key(&path));
    }

    #[test]
    fn test_file_hash_cache_update() {
        let mut file_hashes: HashMap<PathBuf, String> = HashMap::new();

        let path = PathBuf::from("/foo/bar.txt");
        file_hashes.insert(path.clone(), "hash1".to_string());
        file_hashes.insert(path.clone(), "hash2".to_string());

        assert_eq!(file_hashes.get(&path), Some(&"hash2".to_string()));
        assert_eq!(file_hashes.len(), 1);
    }

    #[test]
    fn test_relative_path_conversion() {
        let path = Path::new("/home/user/documents/file.txt");
        let relative = path
            .strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .replace('/', "_");

        assert_eq!(relative, "home_user_documents_file.txt");
    }

    #[test]
    fn test_relative_path_without_leading_slash() {
        let path = Path::new("relative/path/file.txt");
        let relative = path
            .strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .replace('/', "_");

        assert_eq!(relative, "relative_path_file.txt");
    }

    #[test]
    fn test_stream_name_format() {
        let path = Path::new("/home/user/file.txt");
        let relative = path
            .strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .replace('/', "_");
        let stream_name = format!("files/{}.bin", relative);

        assert_eq!(stream_name, "files/home_user_file.txt.bin");
    }

    #[test]
    fn test_glob_pattern_matching() {
        let pattern = glob::Pattern::new("*.txt").unwrap();

        assert!(pattern.matches("file.txt"));
        assert!(pattern.matches("document.txt"));
        assert!(!pattern.matches("file.log"));
        assert!(!pattern.matches("file.txt.bak"));
    }

    #[test]
    fn test_glob_pattern_wildcard() {
        let pattern = glob::Pattern::new("test_*").unwrap();

        assert!(pattern.matches("test_file"));
        assert!(pattern.matches("test_123"));
        assert!(!pattern.matches("file_test"));
    }

    #[test]
    fn test_glob_pattern_recursive() {
        let pattern = glob::Pattern::new("**/*.rs").unwrap();

        assert!(pattern.matches("src/main.rs"));
        assert!(pattern.matches("a/b/c/lib.rs"));
    }

    #[test]
    fn test_sync_interval_duration() {
        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
            sync_interval_secs: 300,
            max_file_size: 1024,
            watch_changes: false,
        };

        let interval = Duration::from_secs(config.sync_interval_secs);
        assert_eq!(interval, Duration::from_secs(300));
        assert_eq!(interval.as_secs(), 300);
    }

    #[test]
    fn test_path_exists_check() {
        let dir = tempdir().unwrap();
        let existing = dir.path().join("exists.txt");
        let nonexistent = dir.path().join("nonexistent.txt");

        fs::write(&existing, "content").unwrap();

        assert!(existing.exists());
        assert!(!nonexistent.exists());
    }

    #[test]
    fn test_known_hash_value() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("known.txt");
        fs::write(&file_path, "hello world").unwrap();

        let hash = hash_file(&file_path).unwrap();
        // SHA256 of "hello world"
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    #[test]
    fn test_hash_newline_sensitive() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("no_newline.txt");
        let file2 = dir.path().join("with_newline.txt");

        fs::write(&file1, "hello").unwrap();
        fs::write(&file2, "hello\n").unwrap();

        let hash1 = hash_file(&file1).unwrap();
        let hash2 = hash_file(&file2).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_pathbuf_hashmap_key() {
        let mut map: HashMap<PathBuf, String> = HashMap::new();

        let path1 = PathBuf::from("/a/b/c");
        let path2 = PathBuf::from("/a/b/c");
        let path3 = PathBuf::from("/a/b/d");

        map.insert(path1.clone(), "value1".to_string());

        assert!(map.contains_key(&path2));
        assert!(!map.contains_key(&path3));
    }

    #[test]
    fn test_walkdir_file_type_filter() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();
        fs::write(subdir.join("nested.txt"), "nested").unwrap();

        let files: Vec<_> = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_max_file_size_boundary() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("boundary.txt");
        let max_size = 100u64;

        // File exactly at max size
        fs::write(&file_path, vec![0u8; max_size as usize]).unwrap();

        let config = DirectorySyncConfig {
            paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
            sync_interval_secs: 300,
            max_file_size: max_size,
            watch_changes: false,
        };

        // Exactly at limit should still sync
        assert!(should_sync_file(&file_path, &config));

        // One byte over should not sync
        fs::write(&file_path, vec![0u8; (max_size + 1) as usize]).unwrap();
        assert!(!should_sync_file(&file_path, &config));
    }
}
