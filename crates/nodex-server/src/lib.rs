//! # Nodex server
//!
//! A local HTTP server that serves the web UI and exposes the project over a small REST API and
//! a WebSocket for live updates. It binds to `127.0.0.1` only and protects every API route with
//! a per-session token plus `Host`/`Origin` checks (DNS-rebinding defense), because later
//! milestones let it start processes.

pub mod adapters;
pub mod api;
pub mod assets;
pub mod builds;
pub mod debugger;
pub mod environment;
pub mod logging;
pub mod projects;
pub mod runner;
pub mod samples;
pub mod security;
pub mod semantic_worker;
pub mod server;
pub mod settings;
pub mod uml;
pub mod unity;
pub mod watch;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, mpsc};

use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use nodex_core::{Catalog, Project};
use nodex_lang_api::LanguageFrontend;
use serde::Serialize;
use tokio::sync::broadcast;

pub use server::{ServerHandle, ServerOptions, frontends, start};

/// Events pushed to every connected browser.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data", rename_all = "kebab-case")]
pub enum ServerEvent {
    /// Files changed on disk and were re-lowered.
    GraphChanged { files: Vec<String> },
    /// Semantic analysis progressed (loading, ready, unavailable) or produced new facts.
    /// `graph` is false when only the status changed and the graphs are the same.
    SemanticChanged { graph: bool },
    /// The run state changed (preparing, running, exited, failed).
    RunStatus(runner::RunStatus),
    /// New lines of build or program output.
    RunOutput { lines: Vec<runner::OutputLine> },
    /// Live execution changes since the previous frame.
    TraceFrame(Box<nodex_trace::TraceFrame>),
    /// Another project is being loaded (from the Open dialog).
    ProjectLoading { path: String },
    /// How far loading got: `phase` is `scanning` (`done` files found, `total` 0) or `reading`
    /// (`done` of `total` files read).
    ProjectProgress { phase: String, done: usize, total: usize },
    /// The project changed: everything shown must be fetched again.
    ProjectChanged,
    /// Loading another project failed; the current one stays.
    ProjectFailed { error: String },
    /// A debug adapter is being downloaded (or finished).
    AdapterProgress(adapters::AdapterProgress),
    /// The debugger's state changed.
    DebugStatus(debugger::DebugStatus),
    /// The program paused: where, and its variables.
    DebugStopped(Box<debugger::DebugStopped>),
    /// The program runs again after a pause.
    DebugContinued,
}

pub struct AppState {
    pub project: Mutex<Project>,
    /// Every language frontend of this binary (to open other projects).
    pub frontends: Vec<Arc<dyn LanguageFrontend>>,
    /// Bumped when another project is opened; work started for an older project is dropped.
    pub generation: AtomicU64,
    /// Watches the current project's folder.
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
    pub token: String,
    pub port: u16,
    /// Extra origins allowed to call the API (the Vite dev server during development).
    pub dev_origins: Vec<String>,
    pub events: broadcast::Sender<ServerEvent>,
    pub catalog: Catalog,
    pub layout_lock: Mutex<()>,
    /// Channels to the semantic workers (changed absolute paths).
    pub semantic_tx: Mutex<Vec<mpsc::Sender<Vec<PathBuf>>>>,
    pub run: runner::RunManager,
    pub debug: debugger::DebugManager,
    /// Recent UML diagrams.
    pub uml: uml::UmlCache,
    /// One structure build at a time ([`builds::graphs`]).
    pub build_lock: tokio::sync::Mutex<()>,
    /// JSON of the current structure graph and knowledge.
    pub json: builds::JsonCache,
    /// The folder being opened, if any (told to browsers that connect meanwhile).
    pub loading: Mutex<Option<String>>,
    /// The runtime the server started on, for background builds started from other threads.
    pub runtime: Option<tokio::runtime::Handle>,
}

impl AppState {
    pub fn new(project: Project, token: String, port: u16, dev_origins: Vec<String>) -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        let frontends = project.frontends();
        Arc::new(Self {
            project: Mutex::new(project),
            frontends,
            generation: AtomicU64::new(0),
            watcher: Mutex::new(None),
            token,
            port,
            dev_origins,
            events,
            catalog: Catalog::embedded(),
            layout_lock: Mutex::new(()),
            semantic_tx: Mutex::new(Vec::new()),
            run: runner::RunManager::default(),
            debug: debugger::DebugManager::default(),
            uml: uml::UmlCache::default(),
            build_lock: tokio::sync::Mutex::new(()),
            json: builds::JsonCache::default(),
            loading: Mutex::new(None),
            runtime: tokio::runtime::Handle::try_current().ok(),
        })
    }

    /// Saved node positions of the current project.
    pub fn layout_file(&self) -> PathBuf {
        self.project.lock().map(|p| p.root().join(".nodex").join("layout.json")).unwrap_or_default()
    }

    /// Stops everything that works on the project (run, debug session, semantic workers, file
    /// watcher) before the server exits.
    pub fn close(&self) {
        self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.run.stop();
        let _ = self.debug.command(debugger::DebugCommand::Stop);
        // Semantic workers end (and stop their sidecars) when their channel closes.
        if let Ok(mut senders) = self.semantic_tx.lock() {
            senders.clear();
        }
        if let Ok(mut w) = self.watcher.lock() {
            *w = None;
        }
    }

    pub fn read_layouts(&self) -> BTreeMap<String, serde_json::Value> {
        std::fs::read_to_string(self.layout_file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
}

/// Builds the application router.
pub fn router(state: Arc<AppState>) -> Router {
    // Routes about the open folder answer 409 on the welcome screen (no folder yet).
    let project = Router::new()
        .route("/api/structure", get(api::structure))
        .route("/api/knowledge", get(api::knowledge))
        .route("/api/uml", get(uml::diagram))
        .route("/api/uml/availability", get(uml::availability))
        .route("/api/flow", get(api::flow))
        .route("/api/flows", get(api::flows))
        .route("/api/source", get(api::source))
        .route("/api/layout", get(api::get_layout).put(api::put_layout))
        .route("/api/debug/breakpoints", get(api::get_breakpoints).put(api::put_breakpoints))
        .route("/api/unity", get(api::unity_status))
        .route("/api/unity/package", post(api::unity_package))
        .route("/api/unity/trace-off", post(api::unity_trace_off))
        .route_layer(middleware::from_fn_with_state(state.clone(), security::require_project));

    let api = Router::new()
        .merge(project)
        .route("/api/project", get(api::project))
        .route("/api/project/open", post(api::project_open))
        .route("/api/project/close", post(api::project_close))
        .route("/api/settings", get(api::get_settings).put(api::put_settings))
        .route("/api/samples", get(api::samples))
        .route("/api/samples/open", post(api::sample_open))
        .route("/api/fs/list", get(api::fs_list))
        .route("/api/recent", get(api::recent))
        .route("/api/catalog", get(api::catalog))
        .route("/api/environment", get(api::environment))
        .route("/api/run", get(api::run_snapshot).post(api::run_start))
        .route("/api/run/stop", post(api::run_stop))
        .route("/api/trace/steps", get(api::trace_steps))
        .route("/api/trace/calls", get(api::trace_calls))
        .route("/api/trace/tree", get(api::trace_tree))
        .route("/api/debug", get(api::debug_snapshot))
        .route("/api/debug/start", post(api::debug_start))
        .route("/api/debug/command", post(api::debug_command))
        .route("/api/debug/variables", get(api::debug_variables))
        .route("/api/debug/scopes", get(api::debug_scopes))
        .route("/api/debug/evaluate", post(api::debug_evaluate))
        .route("/api/debug/adapters", get(api::debug_adapters))
        .route("/api/debug/adapters/install", post(api::debug_adapter_install))
        .route("/ws", get(api::ws))
        .route_layer(middleware::from_fn_with_state(state.clone(), security::require_token));

    Router::new()
        .merge(api)
        // Browser apps under trace send events here; the run token authorizes them.
        .route("/trace", get(api::trace_ws))
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
