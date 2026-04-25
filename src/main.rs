mod system;
mod hook_server;
mod web_server;
mod masker;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use storage::StorageHandle;

#[derive(Parser)]
#[command(name = "claude-guardian", version, about = "Local observer daemon for Claude Code hooks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Start,
    Stop,
    Logs,
    Run,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Start => {
            system::boot::install()?;
            let binary = std::env::current_exe()?;
            let log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/claude-guardian.log")?;
            std::process::Command::new(binary)
                .arg("run")
                .stdin(std::process::Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?;
            println!("claude-guardian started. Logs: /tmp/claude-guardian.log");
        }
        Command::Stop => {
            system::boot::uninstall()?;
            println!("claude-guardian stopped and removed from boot.");
        }
        Command::Logs => {
            open::that("http://localhost:7422")?;
        }
        Command::Run => {
            run_daemon()?;
        }
    }

    Ok(())
}

fn run_daemon() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // SSE
    let (tx, _) = broadcast::channel::<serde_json::Value>(1024);
    let tx = Arc::new(tx);

    // Storage
    let db = rt.block_on(async { StorageHandle::open("claude-guardian.db") })?;
    let db = Arc::new(db);

    // Hook Server
    let hook_tx = Arc::clone(&tx);
    let hook_db = Arc::clone(&db);
    rt.spawn(async move {
        if let Err(e) = hook_server::server::serve(hook_tx, hook_db).await {
            tracing::error!("hook receiver error: {e}");
        }
    });

    // Web Server
    let web_tx = Arc::clone(&tx);
    let web_db = Arc::clone(&db);
    rt.spawn(async move {
        if let Err(e) = web_server::server::serve(web_tx, web_db).await {
            tracing::error!("web server error: {e}");
        }
    });

    system::tray::run_event_loop(rt)?;

    Ok(())
}
