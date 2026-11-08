//! Starting and stopping the server inside a host process: the `nodex` command line or the
//! desktop app, which embeds it and shows the UI in its own window.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::{AppState, new_token, projects, router, semantic_worker, watch};

/// How long [`ServerHandle::shutdown`] waits for runs, debug sessions and open connections.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// Every language frontend compiled into Nodex.
pub fn frontends() -> Vec<Arc<dyn LanguageFrontend>> {
    vec![
        Arc::new(nodex_lang_csharp::CSharpFrontend),
        Arc::new(nodex_lang_typescript::TypeScriptFrontend),
        Arc::new(nodex_lang_rust::RustFrontend),
    ]
}

pub struct ServerOptions {
    /// Folder to open at start-up; `None` starts on the welcome screen.
    pub project: Option<PathBuf>,
    /// First port to try on 127.0.0.1 (the next 19 are tried after it); 0 takes any free port.
    pub port: u16,
    /// Extra origins allowed to call the API (the Vite dev server).
    pub dev_origins: Vec<String>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self { project: None, port: 0, dev_origins: Vec::new() }
    }
}

/// A running server.
pub struct ServerHandle {
    pub state: Arc<AppState>,
    pub port: u16,
    pub token: String,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<std::io::Result<()>>,
}

/// Loads the project (if any), binds the port and serves the API and UI in the background.
pub async fn start(opts: ServerOptions) -> anyhow::Result<ServerHandle> {
    let project = match opts.project {
        Some(path) => {
            let started = Instant::now();
            let project = tokio::task::spawn_blocking(move || Project::open(&path, frontends())).await??;
            let summary = project.summary();
            tracing::info!("loaded {} ({} files) in {:?}", summary.name, summary.files.len(), started.elapsed());
            project
        }
        None => Project::empty(frontends()),
    };

    let (listener, port) = bind(opts.port).await?;
    let token = new_token();
    let state = AppState::new(project, token.clone(), port, opts.dev_origins);
    let open = state.project.lock().map(|p| p.is_open()).unwrap_or(false);
    if open {
        let watcher = watch::spawn(state.clone()).context("cannot watch the project folder")?;
        if let Ok(mut w) = state.watcher.lock() {
            *w = Some(watcher);
        }
        semantic_worker::spawn(&state);
        if let Ok(project) = state.project.lock() {
            projects::remember(&project);
        }
    }

    let (stop, stopped) = oneshot::channel::<()>();
    let app = router(state.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    tracing::info!(port, "Nodex server listening");
    Ok(ServerHandle { state, port, token, stop: Some(stop), task })
}

impl ServerHandle {
    /// The UI's address, without the token.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// The UI's address with the session token, for a browser.
    pub fn url_with_token(&self) -> String {
        format!("http://127.0.0.1:{}/?token={}", self.port, self.token)
    }

    /// Waits until the server stops on its own (an I/O error).
    pub async fn wait(&mut self) -> anyhow::Result<()> {
        (&mut self.task).await??;
        Ok(())
    }

    /// Stops the run, the debug session, the semantic workers and the file watcher, then the
    /// HTTP server. Waits at most a few seconds.
    pub async fn shutdown(mut self) {
        tracing::info!("shutting down");
        self.state.close();
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while (self.state.run.is_busy() || self.state.debug.is_busy()) && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        // Open WebSockets can keep a graceful shutdown waiting; they are dropped with the task.
        let remaining = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(200));
        if tokio::time::timeout(remaining, &mut self.task).await.is_err() {
            self.task.abort();
        }
    }
}

/// Binds 127.0.0.1 on `port` or one of the next 19 (any free port for 0).
async fn bind(port: u16) -> anyhow::Result<(tokio::net::TcpListener, u16)> {
    let candidates: Vec<u16> = if port == 0 { vec![0] } else { (port..port.saturating_add(20)).collect() };
    for candidate in candidates {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, candidate));
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            let bound = listener.local_addr()?.port();
            return Ok((listener, bound));
        }
    }
    anyhow::bail!("no free port between {port} and {}", port.saturating_add(19))
}
