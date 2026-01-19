//! System information and monitoring.

use anyhow::Result;
use serde_json::{json, Value};
use sysinfo::{Disks, Networks, System};
use tracing::debug;

/// Get comprehensive system information.
pub async fn get_info() -> Result<Value> {
    debug!("Getting system info");

    let result = tokio::task::spawn_blocking(|| -> Result<Value> {
        let mut sys = System::new_all();

        // Wait a bit for CPU usage to be accurate
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu_usage();

        // CPU info
        let cpu_count = sys.cpus().len();
        let cpu_usage: f32 =
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32;
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();

        // Memory info
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let total_swap = sys.total_swap();
        let used_swap = sys.used_swap();

        // Disk info
        let disks = Disks::new_with_refreshed_list();
        let disk_info: Vec<Value> = disks
            .iter()
            .map(|d| {
                json!({
                    "name": d.name().to_string_lossy(),
                    "mount_point": d.mount_point().to_string_lossy(),
                    "total_space": d.total_space(),
                    "available_space": d.available_space(),
                    "file_system": String::from_utf8_lossy(d.file_system().as_encoded_bytes()).to_string(),
                    "is_removable": d.is_removable(),
                })
            })
            .collect();

        // Network info
        let networks = Networks::new_with_refreshed_list();
        let network_info: Vec<Value> = networks
            .iter()
            .map(|(name, data)| {
                json!({
                    "name": name,
                    "received": data.total_received(),
                    "transmitted": data.total_transmitted(),
                    "mac_address": data.mac_address().to_string(),
                })
            })
            .collect();

        Ok(json!({
            "hostname": System::host_name().unwrap_or_default(),
            "os": {
                "name": System::name().unwrap_or_default(),
                "version": System::os_version().unwrap_or_default(),
                "kernel_version": System::kernel_version().unwrap_or_default(),
                "arch": std::env::consts::ARCH,
            },
            "cpu": {
                "brand": cpu_brand,
                "cores": cpu_count,
                "usage_percent": cpu_usage,
            },
            "memory": {
                "total": total_memory,
                "used": used_memory,
                "available": total_memory - used_memory,
                "usage_percent": (used_memory as f64 / total_memory as f64) * 100.0,
            },
            "swap": {
                "total": total_swap,
                "used": used_swap,
            },
            "disks": disk_info,
            "network": network_info,
            "uptime": System::uptime(),
        }))
    })
    .await??;

    Ok(result)
}

/// Get list of running processes.
pub async fn get_processes() -> Result<Value> {
    debug!("Getting process list");

    let result = tokio::task::spawn_blocking(|| -> Result<Value> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let processes: Vec<Value> = sys
            .processes()
            .iter()
            .map(|(pid, process)| {
                json!({
                    "pid": pid.as_u32(),
                    "name": process.name().to_string_lossy(),
                    "cpu_usage": process.cpu_usage(),
                    "memory": process.memory(),
                    "status": format!("{:?}", process.status()),
                })
            })
            .collect();

        Ok(json!({
            "processes": processes,
            "count": processes.len(),
        }))
    })
    .await??;

    Ok(result)
}

/// Get GPU information using platform-specific methods.
#[allow(unused_mut)]
pub async fn get_gpu_info() -> Result<Value> {
    debug!("Getting GPU info");

    let result = tokio::task::spawn_blocking(|| -> Result<Value> {
        let mut gpus = Vec::new();

        // Try nvidia-smi for NVIDIA GPUs
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,memory.used,utilization.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split(", ").collect();
                    if parts.len() >= 4 {
                        gpus.push(json!({
                            "name": parts[0],
                            "vendor": "NVIDIA",
                            "memory_total_mb": parts[1].parse::<u64>().unwrap_or(0),
                            "memory_used_mb": parts[2].parse::<u64>().unwrap_or(0),
                            "utilization_percent": parts[3].parse::<f32>().unwrap_or(0.0),
                        }));
                    }
                }
            }
        }

        // macOS: system_profiler
        #[cfg(target_os = "macos")]
        if gpus.is_empty() {
            if let Ok(output) = std::process::Command::new("system_profiler")
                .args(["SPDisplaysDataType", "-json"])
                .output()
            {
                if output.status.success() {
                    if let Ok(data) = serde_json::from_slice::<Value>(&output.stdout) {
                        if let Some(displays) =
                            data.get("SPDisplaysDataType").and_then(|d| d.as_array())
                        {
                            for display in displays {
                                gpus.push(json!({
                                    "name": display.get("sppci_model").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "vendor": display.get("sppci_vendor").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                    "vram": display.get("spdisplays_vram").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(json!({
            "gpus": gpus,
            "count": gpus.len(),
        }))
    })
    .await??;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_info_returns_valid_structure() {
        let result = get_info().await;
        assert!(result.is_ok());

        let info = result.unwrap();

        // Check top-level fields exist
        assert!(info.get("hostname").is_some());
        assert!(info.get("os").is_some());
        assert!(info.get("cpu").is_some());
        assert!(info.get("memory").is_some());
        assert!(info.get("swap").is_some());
        assert!(info.get("disks").is_some());
        assert!(info.get("network").is_some());
        assert!(info.get("uptime").is_some());
    }

    #[tokio::test]
    async fn test_get_info_os_fields() {
        let result = get_info().await.unwrap();
        let os = result.get("os").unwrap();

        assert!(os.get("name").is_some());
        assert!(os.get("version").is_some());
        assert!(os.get("kernel_version").is_some());
        assert!(os.get("arch").is_some());
    }

    #[tokio::test]
    async fn test_get_info_cpu_fields() {
        let result = get_info().await.unwrap();
        let cpu = result.get("cpu").unwrap();

        assert!(cpu.get("brand").is_some());
        assert!(cpu.get("cores").is_some());
        assert!(cpu.get("usage_percent").is_some());

        // Cores should be > 0
        let cores = cpu.get("cores").unwrap().as_u64().unwrap();
        assert!(cores > 0);
    }

    #[tokio::test]
    async fn test_get_info_memory_fields() {
        let result = get_info().await.unwrap();
        let memory = result.get("memory").unwrap();

        assert!(memory.get("total").is_some());
        assert!(memory.get("used").is_some());
        assert!(memory.get("available").is_some());
        assert!(memory.get("usage_percent").is_some());

        // Total memory should be > 0
        let total = memory.get("total").unwrap().as_u64().unwrap();
        assert!(total > 0);
    }

    #[tokio::test]
    async fn test_get_processes_returns_valid_structure() {
        let result = get_processes().await;
        assert!(result.is_ok());

        let procs = result.unwrap();
        assert!(procs.get("processes").is_some());
        assert!(procs.get("count").is_some());

        // Should have at least one process (ourselves)
        let count = procs.get("count").unwrap().as_u64().unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_get_gpu_info_returns_valid_structure() {
        let result = get_gpu_info().await;
        assert!(result.is_ok());

        let gpu_info = result.unwrap();
        assert!(gpu_info.get("gpus").is_some());
        assert!(gpu_info.get("count").is_some());

        // gpus should be an array
        assert!(gpu_info.get("gpus").unwrap().is_array());
    }
}
