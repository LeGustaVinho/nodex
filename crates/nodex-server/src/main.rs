//! `nodex` command-line entry point.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use nodex_server::{ServerOptions, logging, start};

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
        /// Project folder: a .NET solution or project, a Unity project, a TypeScript/JavaScript
        /// project or a Cargo package/workspace. Defaults to the current folder; another one can
        /// be opened from the web interface.
        #[arg(default_value = ".")]
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log = logging::init(true);
    nodex_lang_api::process::kill_children_on_exit();

    match Cli::parse().command {
        Command::Open { path, port, no_browser, dev_origin } => open(path, port, no_browser, dev_origin).await,
    }
}

async fn open(path: PathBuf, port: u16, no_browser: bool, dev_origins: Vec<String>) -> anyhow::Result<()> {
    let mut server = start(ServerOptions { project: Some(path), port, dev_origins }).await?;
    let url = server.url_with_token();
    println!("Nodex is running at {url}");
    println!("Dev UI (with --dev-origin http://localhost:5173): http://localhost:5173/?token={}", server.token);
    if !no_browser {
        let _ = open::that(&url);
    }
    let ended = tokio::select! {
        result = server.wait() => Some(result),
        _ = tokio::signal::ctrl_c() => None,
    };
    match ended {
        Some(result) => result,
        None => {
            server.shutdown().await;
            Ok(())
        }
    }
}
