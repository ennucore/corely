mod camera;
mod connection;
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
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "corely")]
#[command(about = "Corely remote worker agent", long_about = None)]
struct Args {
    /// Server WebSocket URL (e.g., ws://localhost:8000/ws/worker)
    #[arg(short, long, required_unless_present = "uninstall")]
    server: Option<String>,

    /// Authentication token
    #[arg(short, long, required_unless_present = "uninstall")]
    token: Option<String>,

    /// Install as system service and request permissions
    #[arg(long)]
    install: bool,

    /// Completely uninstall the worker from this system
    #[arg(long)]
    uninstall: bool,

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

    // Get required args (safe to unwrap since clap enforces them unless uninstall)
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

    // Load plugins if specified
    let plugins = if let Some(tools_path) = args.tools {
        info!("Loading plugins from: {:?}", tools_path);
        plugins::load_plugins(&tools_path)?
    } else {
        Vec::new()
    };

    // Start connection loop
    connection::run(&server, &token, &worker_name, plugins).await
}
