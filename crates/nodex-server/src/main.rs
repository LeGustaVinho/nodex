//! `nodex` command-line entry point.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_server::{AppState, new_token, router, semantic_worker, watch};

#[derive(Parser)]
#[command(name = "nodex", version, about = "Visualize code as language-agnostic node graphs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Open a project folder in the Nodex web UI.
    Open {
        /// Project folder (a .NET solution/project folder or a Unity project).
        path: PathBuf,
        /// Port to listen on (127.0.0.1 only). The next free port is used if it is taken.
        #[arg(long, default_value_t = 7420)]
        port: u16,
        /// Do not open a browser window.
        #[arg(long)]
        no_browser: bool,
        /// Allow API calls from this origin (the Vite dev server), for example http://localhost:5173.
        #[arg(long)]
        dev_origin: Vec<String>,
    },
}

/// Every language frontend compiled into this binary.
fn frontends() -> Vec<Arc<dyn LanguageFrontend>> {
    vec![Arc::new(nodex_lang_csharp::CSharpFrontend)]
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "nodex=info,nodex_server=info".into()))
        .init();

    match Cli::parse().command {
        Command::Open { path, port, no_browser, dev_origin } => open(path, port, no_browser, dev_origin).await,
    }
}

async fn open(path: PathBuf, port: u16, no_browser: bool, dev_origins: Vec<String>) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let project = tokio::task::spawn_blocking(move || Project::open(&path, frontends())).await??;
    let summary = project.summary();
    tracing::info!(
        "loaded {} ({} files) in {:?}",
        summary.name,
        summary.files.len(),
        started.elapsed()
    );

    let (listener, port) = bind(port).await?;
    let token = new_token();
    let state = AppState::new(project, token.clone(), port, dev_origins);
    let _watcher = watch::spawn(state.clone()).context("cannot watch the project folder")?;
    semantic_worker::spawn(&state);

    let url = format!("http://127.0.0.1:{port}/?token={token}");
    println!("Nodex is running at {url}");
    println!("Dev UI (with --dev-origin http://localhost:5173): http://localhost:5173/?token={token}");
    if !no_browser {
        let _ = open::that(&url);
    }
    axum::serve(listener, router(state)).await?;
    Ok(())
}

async fn bind(port: u16) -> anyhow::Result<(tokio::net::TcpListener, u16)> {
    for candidate in port..port.saturating_add(20) {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, candidate));
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            return Ok((listener, candidate));
        }
    }
    anyhow::bail!("no free port between {port} and {}", port.saturating_add(19))
}
