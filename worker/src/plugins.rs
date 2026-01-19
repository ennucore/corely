use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::fs;
use tracing::{debug, error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub path: String,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub tools: Vec<PluginTool>,
    pub runtime: String, // "python", "node", "binary"
    pub entrypoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

pub fn load_plugins(tools_path: &Path) -> Result<Vec<Plugin>> {
    let mut plugins = Vec::new();

    if !tools_path.exists() {
        return Ok(plugins);
    }

    for entry in std::fs::read_dir(tools_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let manifest_path = path.join("manifest.json");
            if manifest_path.exists() {
                match load_plugin(&path, &manifest_path) {
                    Ok(plugin) => {
                        info!("Loaded plugin: {}", plugin.name);
                        plugins.push(plugin);
                    }
                    Err(e) => {
                        error!("Failed to load plugin from {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    Ok(plugins)
}

fn load_plugin(plugin_path: &Path, manifest_path: &Path) -> Result<Plugin> {
    let manifest_content = std::fs::read_to_string(manifest_path)?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_content)?;

    Ok(Plugin {
        name: manifest.name.clone(),
        path: plugin_path.to_string_lossy().to_string(),
        manifest,
    })
}

pub async fn invoke(
    plugins: &[Plugin],
    plugin_name: &str,
    tool_name: &str,
    params: Value,
) -> Result<Value> {
    debug!(
        "Invoking plugin: {} tool: {} with params: {:?}",
        plugin_name, tool_name, params
    );

    let plugin = plugins
        .iter()
        .find(|p| p.name == plugin_name)
        .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_name))?;

    // Verify tool exists
    if !plugin.manifest.tools.iter().any(|t| t.name == tool_name) {
        return Err(anyhow!(
            "Tool '{}' not found in plugin '{}'",
            tool_name,
            plugin_name
        ));
    }

    // Prepare the request
    let request = json!({
        "jsonrpc": "2.0",
        "id": uuid::Uuid::new_v4().to_string(),
        "method": tool_name,
        "params": params,
    });

    // Determine how to run the plugin
    let (program, args): (String, Vec<String>) = match plugin.manifest.runtime.as_str() {
        "python" => (
            "python3".to_string(),
            vec![
                "-u".to_string(),
                format!("{}/{}", plugin.path, plugin.manifest.entrypoint),
            ],
        ),
        "node" => (
            "node".to_string(),
            vec![format!("{}/{}", plugin.path, plugin.manifest.entrypoint)],
        ),
        "binary" => (
            format!("{}/{}", plugin.path, plugin.manifest.entrypoint),
            vec![],
        ),
        _ => return Err(anyhow!("Unknown runtime: {}", plugin.manifest.runtime)),
    };

    // Spawn the plugin process
    let mut child = Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Send the request
    if let Some(mut stdin) = child.stdin.take() {
        let request_str = serde_json::to_string(&request)?;
        stdin.write_all(request_str.as_bytes())?;
        stdin.write_all(b"\n")?;
    }

    // Read the response
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("No stdout"))?;
    let reader = BufReader::new(stdout);

    let mut response_line = String::new();
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('{') {
            response_line = line;
            break;
        }
    }

    child.wait()?;

    if response_line.is_empty() {
        return Err(anyhow!("No response from plugin"));
    }

    let response: Value = serde_json::from_str(&response_line)?;

    if let Some(error) = response.get("error") {
        return Err(anyhow!(
            "Plugin error: {}",
            error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error")
        ));
    }

    Ok(response.get("result").cloned().unwrap_or(json!(null)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_manifest_deserialize() {
        let json = r#"{
            "name": "test-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "tools": [
                {
                    "name": "test_tool",
                    "description": "A test tool",
                    "parameters": {"type": "object"}
                }
            ],
            "runtime": "python",
            "entrypoint": "main.py"
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.runtime, "python");
        assert_eq!(manifest.entrypoint, "main.py");
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "test_tool");
    }

    #[test]
    fn test_load_plugins_empty_dir() {
        let dir = tempdir().unwrap();
        let plugins = load_plugins(dir.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_plugins_nonexistent_dir() {
        let plugins = load_plugins(std::path::Path::new("/nonexistent/path")).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_plugins_with_valid_plugin() {
        let dir = tempdir().unwrap();
        let plugin_dir = dir.path().join("my-plugin");
        fs::create_dir(&plugin_dir).unwrap();

        let manifest = r#"{
            "name": "my-plugin",
            "version": "0.1.0",
            "tools": [],
            "runtime": "python",
            "entrypoint": "main.py"
        }"#;
        fs::write(plugin_dir.join("manifest.json"), manifest).unwrap();

        let plugins = load_plugins(dir.path()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "my-plugin");
    }

    #[test]
    fn test_load_plugins_skips_invalid_manifest() {
        let dir = tempdir().unwrap();
        let plugin_dir = dir.path().join("bad-plugin");
        fs::create_dir(&plugin_dir).unwrap();

        // Invalid JSON
        fs::write(plugin_dir.join("manifest.json"), "not valid json").unwrap();

        let plugins = load_plugins(dir.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_invoke_plugin_not_found() {
        let plugins = vec![];
        let result = invoke(&plugins, "nonexistent", "tool", json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plugin not found"));
    }

    #[tokio::test]
    async fn test_invoke_tool_not_found() {
        let plugin = Plugin {
            name: "test".to_string(),
            path: "/tmp".to_string(),
            manifest: PluginManifest {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                tools: vec![],
                runtime: "python".to_string(),
                entrypoint: "main.py".to_string(),
            },
        };
        let plugins = vec![plugin];

        let result = invoke(&plugins, "test", "nonexistent_tool", json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
