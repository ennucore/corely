//! PTY-based interactive terminal sessions.
//!
//! This module provides tmux-like functionality without requiring tmux.
//! Sessions are persistent and can be attached/detached.

use anyhow::{anyhow, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Buffer size for reading PTY output
const READ_BUFFER_SIZE: usize = 4096;

/// Default terminal size
const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 40;

/// A single terminal session
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: std::time::Instant,
    pty_pair: PtyPair,
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
    output_buffer: Vec<u8>,
    cols: u16,
    rows: u16,
}

/// Manager for all terminal sessions
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new terminal session
    pub fn create_session(&mut self, name: Option<String>, shell: Option<String>) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let session_name = name.unwrap_or_else(|| format!("session-{}", &session_id[..8]));

        info!("Creating session: {} ({})", session_name, session_id);

        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows: DEFAULT_ROWS,
            cols: DEFAULT_COLS,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        // Determine shell to use
        let shell_cmd = shell.unwrap_or_else(|| {
            std::env::var("SHELL").unwrap_or_else(|_| {
                if cfg!(target_os = "windows") {
                    "cmd.exe".to_string()
                } else {
                    "/bin/sh".to_string()
                }
            })
        });

        let mut cmd = CommandBuilder::new(&shell_cmd);

        // Set up environment
        cmd.env("TERM", "xterm-256color");
        cmd.env("CORELY_SESSION", &session_id);

        // Spawn the shell
        let _child = pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let reader = pair.master.try_clone_reader()?;

        let session = Session {
            id: session_id.clone(),
            name: session_name,
            created_at: std::time::Instant::now(),
            pty_pair: pair,
            writer,
            reader,
            output_buffer: Vec::with_capacity(READ_BUFFER_SIZE * 10),
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        };

        self.sessions.insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<Value> {
        self.sessions
            .values()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "uptime_secs": s.created_at.elapsed().as_secs(),
                    "size": format!("{}x{}", s.cols, s.rows),
                })
            })
            .collect()
    }

    /// Get a session by ID or name
    pub fn get_session(&mut self, id_or_name: &str) -> Option<&mut Session> {
        // Try by ID first
        if self.sessions.contains_key(id_or_name) {
            return self.sessions.get_mut(id_or_name);
        }

        // Try by name
        let id = self
            .sessions
            .values()
            .find(|s| s.name == id_or_name)
            .map(|s| s.id.clone());

        id.and_then(move |id| self.sessions.get_mut(&id))
    }

    /// Send input to a session
    pub fn send_input(&mut self, session_id: &str, input: &str) -> Result<()> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        session.writer.write_all(input.as_bytes())?;
        session.writer.flush()?;

        Ok(())
    }

    /// Send a special key to a session
    pub fn send_key(&mut self, session_id: &str, key: &str) -> Result<()> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        let bytes: Vec<u8> = match key.to_lowercase().as_str() {
            "enter" => vec![b'\r'],
            "tab" => vec![b'\t'],
            "backspace" => vec![0x7f],
            "escape" | "esc" => vec![0x1b],
            "up" => vec![0x1b, b'[', b'A'],
            "down" => vec![0x1b, b'[', b'B'],
            "right" => vec![0x1b, b'[', b'C'],
            "left" => vec![0x1b, b'[', b'D'],
            "home" => vec![0x1b, b'[', b'H'],
            "end" => vec![0x1b, b'[', b'F'],
            "pageup" => vec![0x1b, b'[', b'5', b'~'],
            "pagedown" => vec![0x1b, b'[', b'6', b'~'],
            "delete" => vec![0x1b, b'[', b'3', b'~'],
            "ctrl-c" => vec![0x03],
            "ctrl-d" => vec![0x04],
            "ctrl-z" => vec![0x1a],
            "ctrl-l" => vec![0x0c],
            _ => {
                // Handle ctrl+letter combinations
                if key.starts_with("ctrl-") && key.len() == 6 {
                    let c = key.chars().last().unwrap();
                    if c.is_ascii_lowercase() {
                        vec![(c as u8) - b'a' + 1]
                    } else {
                        return Err(anyhow!("Unknown key: {}", key));
                    }
                } else {
                    return Err(anyhow!("Unknown key: {}", key));
                }
            }
        };

        session.writer.write_all(&bytes)?;
        session.writer.flush()?;

        Ok(())
    }

    /// Read output from a session
    pub fn read_output(&mut self, session_id: &str) -> Result<String> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        let mut buffer = [0u8; READ_BUFFER_SIZE];

        // Non-blocking read
        loop {
            match session.reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    session.output_buffer.extend_from_slice(&buffer[..n]);
                    if n < READ_BUFFER_SIZE {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }

        // Return buffered output
        let output = String::from_utf8_lossy(&session.output_buffer).to_string();
        session.output_buffer.clear();

        Ok(output)
    }

    /// Get the full output buffer without clearing it
    pub fn peek_output(&self, session_id: &str) -> Result<String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        Ok(String::from_utf8_lossy(&session.output_buffer).to_string())
    }

    /// Resize a session
    pub fn resize(&mut self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        session.pty_pair.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        session.cols = cols;
        session.rows = rows;

        Ok(())
    }

    /// Kill a session
    pub fn kill_session(&mut self, session_id: &str) -> Result<()> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        info!("Killed session: {} ({})", session.name, session.id);

        // Session will be dropped here, cleaning up resources
        Ok(())
    }

    /// Rename a session
    pub fn rename_session(&mut self, session_id: &str, new_name: &str) -> Result<()> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        session.name = new_name.to_string();
        Ok(())
    }
}

// Global session manager
lazy_static::lazy_static! {
    pub static ref SESSION_MANAGER: Arc<Mutex<SessionManager>> =
        Arc::new(Mutex::new(SessionManager::new()));
}

// Public async API

pub async fn create(name: Option<String>, shell: Option<String>) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    let session_id = manager.create_session(name, shell)?;

    Ok(json!({
        "status": "ok",
        "session_id": session_id,
    }))
}

pub async fn list() -> Result<Value> {
    let manager = SESSION_MANAGER.lock().await;
    let sessions = manager.list_sessions();

    Ok(json!({
        "sessions": sessions,
        "count": sessions.len(),
    }))
}

pub async fn send_input(session_id: &str, input: &str) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    manager.send_input(session_id, input)?;

    Ok(json!({
        "status": "ok",
    }))
}

pub async fn send_key(session_id: &str, key: &str) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    manager.send_key(session_id, key)?;

    Ok(json!({
        "status": "ok",
    }))
}

pub async fn read_output(session_id: &str) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    let output = manager.read_output(session_id)?;

    Ok(json!({
        "output": output,
    }))
}

pub async fn resize(session_id: &str, cols: u16, rows: u16) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    manager.resize(session_id, cols, rows)?;

    Ok(json!({
        "status": "ok",
        "size": format!("{}x{}", cols, rows),
    }))
}

pub async fn kill(session_id: &str) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    manager.kill_session(session_id)?;

    Ok(json!({
        "status": "ok",
    }))
}

pub async fn rename(session_id: &str, new_name: &str) -> Result<Value> {
    let mut manager = SESSION_MANAGER.lock().await;
    manager.rename_session(session_id, new_name)?;

    Ok(json!({
        "status": "ok",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_new() {
        let manager = SessionManager::new();
        assert!(manager.sessions.is_empty());
    }

    #[test]
    fn test_session_manager_list_empty() {
        let manager = SessionManager::new();
        let sessions = manager.list_sessions();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_send_key_escape_sequences() {
        // Test that escape sequences are generated correctly
        let mut manager = SessionManager::new();

        // Create a session first
        let result = manager.create_session(Some("test".to_string()), None);
        if result.is_err() {
            // Skip test if PTY creation fails (e.g., in CI)
            return;
        }
        let session_id = result.unwrap();

        // Test sending keys (these should not error)
        assert!(manager.send_key(&session_id, "enter").is_ok());
        assert!(manager.send_key(&session_id, "tab").is_ok());
        assert!(manager.send_key(&session_id, "ctrl-c").is_ok());

        // Unknown key should error
        assert!(manager.send_key(&session_id, "unknown-key").is_err());

        // Clean up
        let _ = manager.kill_session(&session_id);
    }

    #[test]
    fn test_session_not_found() {
        let mut manager = SessionManager::new();

        assert!(manager.send_input("nonexistent", "test").is_err());
        assert!(manager.send_key("nonexistent", "enter").is_err());
        assert!(manager.read_output("nonexistent").is_err());
        assert!(manager.resize("nonexistent", 80, 24).is_err());
        assert!(manager.kill_session("nonexistent").is_err());
        assert!(manager.rename_session("nonexistent", "new-name").is_err());
    }
}
