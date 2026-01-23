use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use glob::glob as glob_match;
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;
use tokio::fs as async_fs;
use tracing::debug;
use walkdir::WalkDir;

pub async fn read(path: &str, offset: Option<u64>, limit: Option<u64>) -> Result<Value> {
    debug!("Reading file: {}", path);

    let content = async_fs::read_to_string(path).await?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    let start = offset.unwrap_or(0) as usize;
    let count = limit.unwrap_or(u64::MAX) as usize;

    if start >= total_lines {
        return Ok(json!({
            "content": "",
            "total_lines": total_lines,
            "offset": start,
            "lines_returned": 0,
        }));
    }

    let end = std::cmp::min(start + count, total_lines);
    let selected_lines: Vec<&str> = lines[start..end].to_vec();

    // Format with line numbers (1-indexed)
    let formatted = selected_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:6}\t{}", start + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "content": formatted,
        "total_lines": total_lines,
        "offset": start,
        "lines_returned": selected_lines.len(),
    }))
}

pub async fn write(path: &str, content: &str) -> Result<Value> {
    debug!("Writing file: {}", path);

    // Create parent directories if they don't exist
    if let Some(parent) = Path::new(path).parent() {
        async_fs::create_dir_all(parent).await?;
    }

    async_fs::write(path, content).await?;

    Ok(json!({
        "status": "ok",
        "path": path,
        "bytes_written": content.len(),
    }))
}

/// Read a file as binary and return base64-encoded content
pub async fn read_binary(path: &str) -> Result<Value> {
    debug!("Reading binary file: {}", path);

    let data = async_fs::read(path).await?;
    let encoded = BASE64.encode(&data);

    Ok(json!({
        "content": encoded,
        "size": data.len(),
        "encoding": "base64",
    }))
}

/// Write binary content (base64-encoded) to a file
pub async fn write_binary(path: &str, content: &str) -> Result<Value> {
    debug!("Writing binary file: {}", path);

    // Create parent directories if they don't exist
    if let Some(parent) = Path::new(path).parent() {
        async_fs::create_dir_all(parent).await?;
    }

    let data = BASE64.decode(content)?;
    async_fs::write(path, &data).await?;

    Ok(json!({
        "status": "ok",
        "path": path,
        "bytes_written": data.len(),
    }))
}

pub async fn edit(path: &str, old_string: &str, new_string: &str) -> Result<Value> {
    debug!("Editing file: {}", path);

    let content = async_fs::read_to_string(path).await?;

    let count = content.matches(old_string).count();
    if count == 0 {
        return Err(anyhow!("old_string not found in file"));
    }

    if count > 1 {
        return Err(anyhow!(
            "old_string found {} times, must be unique. Provide more context.",
            count
        ));
    }

    let new_content = content.replace(old_string, new_string);
    async_fs::write(path, &new_content).await?;

    Ok(json!({
        "status": "ok",
        "path": path,
        "replacements": 1,
    }))
}

pub async fn glob_search(pattern: &str, base_path: Option<&str>) -> Result<Value> {
    debug!("Glob search: {} in {:?}", pattern, base_path);

    let search_pattern = if let Some(base) = base_path {
        format!("{}/{}", base.trim_end_matches('/'), pattern)
    } else {
        pattern.to_string()
    };

    let mut matches = Vec::new();

    for entry in glob_match(&search_pattern)? {
        match entry {
            Ok(path) => {
                if let Some(path_str) = path.to_str() {
                    matches.push(path_str.to_string());
                }
            }
            Err(e) => {
                debug!("Glob error: {}", e);
            }
        }
    }

    // Sort by modification time (most recent first)
    matches.sort_by(|a, b| {
        let time_a = fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let time_b = fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    Ok(json!({
        "matches": matches,
        "count": matches.len(),
    }))
}

pub async fn grep(pattern: &str, base_path: Option<&str>) -> Result<Value> {
    debug!("Grep search: {} in {:?}", pattern, base_path);

    let regex = Regex::new(pattern)?;
    let search_path = base_path.unwrap_or(".");

    let mut results = Vec::new();

    for entry in WalkDir::new(search_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Skip binary files
        if let Ok(file) = fs::File::open(path) {
            let reader = BufReader::new(file);
            let mut file_matches = Vec::new();

            for (line_num, line) in reader.lines().enumerate() {
                if let Ok(line_content) = line {
                    if regex.is_match(&line_content) {
                        file_matches.push(json!({
                            "line": line_num + 1,
                            "content": line_content,
                        }));
                    }
                }
            }

            if !file_matches.is_empty() {
                results.push(json!({
                    "file": path.to_string_lossy(),
                    "matches": file_matches,
                }));
            }
        }
    }

    Ok(json!({
        "results": results,
        "files_matched": results.len(),
    }))
}

pub async fn stat(path: &str) -> Result<Value> {
    debug!("Stat: {}", path);

    let metadata = async_fs::metadata(path).await?;

    let file_type = if metadata.is_file() {
        "file"
    } else if metadata.is_dir() {
        "directory"
    } else if metadata.is_symlink() {
        "symlink"
    } else {
        "unknown"
    };

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    let accessed = metadata
        .accessed()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    Ok(json!({
        "path": path,
        "type": file_type,
        "size": metadata.len(),
        "modified": modified,
        "created": created,
        "accessed": accessed,
        "readonly": metadata.permissions().readonly(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3").unwrap();

        let result = read(file_path.to_str().unwrap(), None, None).await.unwrap();

        assert_eq!(result["total_lines"], 3);
        assert_eq!(result["lines_returned"], 3);
        assert!(result["content"].as_str().unwrap().contains("line1"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4").unwrap();

        let result = read(file_path.to_str().unwrap(), Some(1), Some(2)).await.unwrap();

        assert_eq!(result["total_lines"], 4);
        assert_eq!(result["lines_returned"], 2);
        assert!(result["content"].as_str().unwrap().contains("line2"));
        assert!(result["content"].as_str().unwrap().contains("line3"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let result = write(file_path.to_str().unwrap(), "hello world").await.unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["bytes_written"], 11);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nested/dir/file.txt");

        let result = write(file_path.to_str().unwrap(), "content").await.unwrap();

        assert_eq!(result["status"], "ok");
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edit.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let result = edit(file_path.to_str().unwrap(), "world", "rust").await.unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["replacements"], 1);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello rust");
    }

    #[tokio::test]
    async fn test_edit_file_not_found() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edit.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let result = edit(file_path.to_str().unwrap(), "notfound", "rust").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_edit_file_multiple_matches() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edit.txt");
        std::fs::write(&file_path, "hello hello hello").unwrap();

        let result = edit(file_path.to_str().unwrap(), "hello", "hi").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("3 times"));
    }

    #[tokio::test]
    async fn test_glob_search() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "").unwrap();
        std::fs::write(dir.path().join("other.rs"), "").unwrap();

        let result = glob_search("*.txt", Some(dir.path().to_str().unwrap())).await.unwrap();

        assert_eq!(result["count"], 2);
        let matches = result["matches"].as_array().unwrap();
        assert!(matches.iter().any(|m| m.as_str().unwrap().contains("file1.txt")));
        assert!(matches.iter().any(|m| m.as_str().unwrap().contains("file2.txt")));
    }

    #[tokio::test]
    async fn test_grep_search() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "hello world\nfoo bar").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "hello rust\nbaz").unwrap();

        let result = grep("hello", Some(dir.path().to_str().unwrap())).await.unwrap();

        assert_eq!(result["files_matched"], 2);
    }

    #[tokio::test]
    async fn test_stat_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("stat_test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        let result = stat(file_path.to_str().unwrap()).await.unwrap();

        assert_eq!(result["type"], "file");
        assert_eq!(result["size"], 12);
        assert!(!result["readonly"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_stat_directory() {
        let dir = tempdir().unwrap();

        let result = stat(dir.path().to_str().unwrap()).await.unwrap();

        assert_eq!(result["type"], "directory");
    }
}
