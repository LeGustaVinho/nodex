//! # Nodex server
//!
//! A local HTTP server that serves the web UI and exposes the project over a small REST API and
//! a WebSocket for live updates. It binds to `127.0.0.1` only and protects every API route with
//! a per-session token plus `Host`/`Origin` checks (DNS-rebinding defense), because later
//! milestones let it start processes.

pub mod api;
pub mod assets;
pub mod security;
pub mod semantic_worker;
pub mod watch;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use axum::Router;
use axum::middleware;
use axum::routing::get;
use nodex_core::{Catalog, Project};
use serde::Serialize;
use tokio::sync::broadcast;

/// Events pushed to every connected browser.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ServerEvent {
    /// Files changed on disk and were re-lowered.
    GraphChanged { files: Vec<String> },
    /// Semantic analysis progressed (loading, ready, unavailable) or produced new facts.
    SemanticChanged,
}

pub struct AppState {
    pub project: Mutex<Project>,
    pub token: String,
    pub port: u16,
    /// Extra origins allowed to call the API (the Vite dev server during development).
    pub dev_origins: Vec<String>,
    pub events: broadcast::Sender<ServerEvent>,
    pub catalog: Catalog,
    pub layout_file: PathBuf,
    pub layout_lock: Mutex<()>,
    /// Channels to the semantic workers (changed absolute paths).
    pub semantic_tx: Mutex<Vec<mpsc::Sender<Vec<PathBuf>>>>,
}

impl AppState {
    pub fn new(project: Project, token: String, port: u16, dev_origins: Vec<String>) -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        let layout_file = project.root().join(".nodex").join("layout.json");
        Arc::new(Self {
            project: Mutex::new(project),
            token,
            port,
            dev_origins,
            events,
            catalog: Catalog::embedded(),
            layout_file,
            layout_lock: Mutex::new(()),
            semantic_tx: Mutex::new(Vec::new()),
        })
    }

    pub fn read_layouts(&self) -> BTreeMap<String, serde_json::Value> {
        std::fs::read_to_string(&self.layout_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
}

/// Builds the application router.
pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/project", get(api::project))
        .route("/api/structure", get(api::structure))
        .route("/api/flow", get(api::flow))
        .route("/api/source", get(api::source))
        .route("/api/catalog", get(api::catalog))
        .route("/api/layout", get(api::get_layout).put(api::put_layout))
        .route("/ws", get(api::ws))
        .route_layer(middleware::from_fn_with_state(state.clone(), security::require_token));

    Router::new()
        .merge(api)
        .fallback(assets::serve)
        .layer(middleware::from_fn_with_state(state.clone(), security::check_host_and_origin))
        .with_state(state)
}

/// Generates a random 32-byte session token, hex-encoded.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS random source is available");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
