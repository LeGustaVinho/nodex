//! REST and WebSocket handlers.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;

type ApiResult = Result<Response, (StatusCode, String)>;

fn not_found(what: &str) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, format!("{what} not found"))
}

pub async fn project(State(state): State<Arc<AppState>>) -> ApiResult {
    let summary = state.project.lock().map_err(poisoned)?.summary();
    Ok(Json(summary).into_response())
}

pub async fn structure(State(state): State<Arc<AppState>>) -> ApiResult {
    let bytes = crate::builds::structure_json(&state).await.map_err(internal)?;
    Ok(([("content-type", "application/json")], bytes).into_response())
}

pub async fn knowledge(State(state): State<Arc<AppState>>) -> ApiResult {
    let bytes = crate::builds::knowledge_json(&state).await.map_err(internal)?;
    Ok(([("content-type", "application/json")], bytes).into_response())
}

#[derive(Deserialize)]
pub struct NavigationParams {
    symbol: String,
    kind: nodex_core::navigation::NavigationKind,
}

pub async fn navigation(State(state): State<Arc<AppState>>, Query(q): Query<NavigationParams>) -> ApiResult {
    use std::sync::atomic::Ordering;
    let generation = state.generation.load(Ordering::SeqCst);
    let project_changed = || (StatusCode::CONFLICT, "project changed during navigation".to_owned());
    loop {
        crate::builds::graphs(&state).await.map_err(internal)?;
        let (query, version) = {
            let project = state.project.lock().map_err(poisoned)?;
            if state.generation.load(Ordering::SeqCst) != generation {
                return Err(project_changed());
            }
            if project.built().is_none() {
                continue;
            }
            (project.navigation(&q.symbol, q.kind).ok_or_else(|| not_found("symbol"))?, project.version())
        };
        let result = tokio::task::spawn_blocking(move || query.resolve()).await.map_err(internal)?;
        let project = state.project.lock().map_err(poisoned)?;
        if state.generation.load(Ordering::SeqCst) != generation {
            return Err(project_changed());
        }
        if project.version() == version {
            drop(project);
            return Ok(Json(result).into_response());
        }
    }
}

#[derive(Deserialize)]
pub struct FlowQuery {
    symbol: String,
}

pub async fn flow(State(state): State<Arc<AppState>>, Query(q): Query<FlowQuery>) -> ApiResult {
    // Enriching a flow needs the built graph (its locator): build it off the lock first.
    crate::builds::graphs(&state).await.map_err(internal)?;
    let flow = state.project.lock().map_err(poisoned)?.flow(&q.symbol).ok_or_else(|| not_found("flow"))?;
    Ok(Json(flow).into_response())
}

pub async fn flows(State(state): State<Arc<AppState>>, RawQuery(query): RawQuery) -> ApiResult {
    // Split the raw list before decoding: symbol IDs can contain commas in parameter lists.
    let symbols = query.as_deref()
        .and_then(|q| q.split('&').find_map(|part| part.strip_prefix("symbols=")))
        .ok_or((StatusCode::BAD_REQUEST, "missing symbols".to_owned()))?;
    crate::builds::graphs(&state).await.map_err(internal)?;
    let mut project = state.project.lock().map_err(poisoned)?;
    let flows: Vec<_> = symbols.split(',').filter(|id| !id.is_empty()).filter_map(|id| {
        let param = format!("symbol={id}");
        let (_, symbol) = form_urlencoded::parse(param.as_bytes()).next()?;
        project.flow(&symbol)
    }).collect();
    Ok(Json(flows).into_response())
}

#[derive(Deserialize)]
pub struct SourceQuery {
    file: String,
}

pub async fn source(State(state): State<Arc<AppState>>, Query(q): Query<SourceQuery>) -> ApiResult {
    let text = state.project.lock().map_err(poisoned)?.source(&q.file).ok_or_else(|| not_found("file"))?;
    let revision = nodex_core::navigation::source_revision(&text);
    Ok(([
        ("content-type", "text/plain; charset=utf-8"),
        ("cache-control", "no-store"),
        ("x-nodex-source-revision", revision.as_str()),
    ], text).into_response())
}

#[derive(Deserialize)]
pub struct OpenRequest {
    path: String,
}

/// Opens another project; browsers get `project-loading`, then `project-changed`.
pub async fn project_open(State(state): State<Arc<AppState>>, Json(req): Json<OpenRequest>) -> ApiResult {
    crate::projects::open(&state, req.path).await.map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    let summary = state.project.lock().map_err(poisoned)?.summary();
    Ok(Json(summary).into_response())
}

pub async fn get_settings() -> ApiResult {
    Ok(Json(crate::settings::load()).into_response())
}

pub async fn put_settings(Json(settings): Json<crate::settings::Settings>) -> ApiResult {
    crate::settings::save(&settings).map_err(internal)?;
    Ok(Json(settings).into_response())
}

/// The sample projects shipped with Nodex.
pub async fn samples() -> ApiResult {
    Ok(Json(crate::samples::list()).into_response())
}

#[derive(Deserialize)]
pub struct SampleRequest {
    id: String,
}

/// Copies a sample to the user's data folder (once) and opens it.
pub async fn sample_open(State(state): State<Arc<AppState>>, Json(req): Json<SampleRequest>) -> ApiResult {
    let dir = tokio::task::spawn_blocking(move || crate::samples::prepare(&req.id))
        .await
        .map_err(internal)?
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    crate::projects::open(&state, dir.to_string_lossy().into_owned()).await.map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    let summary = state.project.lock().map_err(poisoned)?.summary();
    Ok(Json(summary).into_response())
}

/// Closes the folder (back to the welcome screen); browsers get `project-changed`.
pub async fn project_close(State(state): State<Arc<AppState>>) -> ApiResult {
    crate::projects::close(&state).map_err(internal)?;
    let summary = state.project.lock().map_err(poisoned)?.summary();
    Ok(Json(summary).into_response())
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    path: String,
}

/// Subfolders of a folder for the Open dialog (drives and home when `path` is empty).
pub async fn fs_list(Query(q): Query<ListQuery>) -> ApiResult {
    let listing = tokio::task::spawn_blocking(move || crate::projects::list(&q.path))
        .await
        .map_err(internal)?
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    Ok(Json(listing).into_response())
}

pub async fn recent() -> ApiResult {
    Ok(Json(crate::projects::recent()).into_response())
}

/// Toolchains and bundled helpers on this machine (each check starts a process).
pub async fn environment() -> ApiResult {
    let report = tokio::task::spawn_blocking(crate::environment::report).await.map_err(internal)?;
    Ok(Json(report).into_response())
}

pub async fn catalog(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(&state.catalog).into_response())
}

#[derive(Deserialize)]
pub struct LayoutQuery {
    key: String,
}

pub async fn get_layout(State(state): State<Arc<AppState>>, Query(q): Query<LayoutQuery>) -> ApiResult {
    let layouts = state.read_layouts();
    Ok(Json(layouts.get(&q.key).cloned().unwrap_or(serde_json::json!({}))).into_response())
}

/// Saves user-adjusted node positions for one graph to `.nodex/layout.json`.
pub async fn put_layout(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LayoutQuery>,
    Json(positions): Json<BTreeMap<String, Position>>,
) -> ApiResult {
    let _guard = state.layout_lock.lock().map_err(poisoned)?;
    let mut layouts = state.read_layouts();
    if positions.is_empty() {
        layouts.remove(&q.key);
    } else {
        layouts.insert(q.key, serde_json::to_value(positions).map_err(internal)?);
    }
    let file = state.layout_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(internal)?;
    }
    let text = serde_json::to_string_pretty(&layouts).map_err(internal)?;
    std::fs::write(&file, text).map_err(internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize, serde::Serialize)]
pub struct Position {
    x: f64,
    y: f64,
}

pub async fn run_snapshot(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(state.run.snapshot()).into_response())
}

/// Builds, instruments and runs the project; progress arrives over the WebSocket.
pub async fn run_start(State(state): State<Arc<AppState>>) -> ApiResult {
    crate::runner::start(&state).map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(StatusCode::ACCEPTED.into_response())
}

pub async fn run_stop(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(if state.run.stop() { StatusCode::ACCEPTED } else { StatusCode::NO_CONTENT }.into_response())
}

#[derive(Deserialize)]
pub struct StepsQuery {
    symbol: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    5000
}

/// Recorded steps of one function, for replay.
pub async fn trace_steps(State(state): State<Arc<AppState>>, Query(q): Query<StepsQuery>) -> ApiResult {
    let recorder = state.run.recorder().ok_or_else(|| not_found("recording"))?;
    let page = recorder.lock().map_err(poisoned)?.steps(&q.symbol, q.offset, q.limit.min(20_000));
    Ok(Json(page).into_response())
}

#[derive(Deserialize)]
pub struct CallsQuery {
    #[serde(default = "default_calls_limit")]
    limit: usize,
}

fn default_calls_limit() -> usize {
    50_000
}

/// Function executions over time, for the flame chart.
pub async fn trace_calls(State(state): State<Arc<AppState>>, Query(q): Query<CallsQuery>) -> ApiResult {
    let recorder = state.run.recorder().ok_or_else(|| not_found("recording"))?;
    let page = recorder.lock().map_err(poisoned)?.calls(q.limit.min(200_000));
    Ok(Json(page).into_response())
}

#[derive(Deserialize)]
pub struct TreeQuery {
    root: u32,
    #[serde(default = "default_tree_limit")]
    limit: usize,
}

fn default_tree_limit() -> usize {
    5000
}

/// One execution and every call made under it (for sequence diagrams of a real run).
pub async fn trace_tree(State(state): State<Arc<AppState>>, Query(q): Query<TreeQuery>) -> ApiResult {
    let recorder = state.run.recorder().ok_or_else(|| not_found("recording"))?;
    let page = recorder.lock().map_err(poisoned)?.span_tree(q.root, q.limit.min(50_000));
    if page.spans.is_empty() {
        return Err(not_found("call"));
    }
    Ok(Json(page).into_response())
}

#[derive(Deserialize)]
pub struct TraceQuery {
    run: String,
}

/// Trace events from browser pages: binary messages, each a frame of the trace protocol.
/// Authorized by the current run's token (pages are served by the app's own dev server, so
/// their origin is not the Nodex UI).
pub async fn trace_ws(State(state): State<Arc<AppState>>, Query(q): Query<TraceQuery>, upgrade: WebSocketUpgrade) -> Response {
    let Some(recorder) = state.run.recorder_for(&q.run) else {
        return (StatusCode::UNAUTHORIZED, "no run with this token").into_response();
    };
    upgrade.on_upgrade(move |mut socket| async move {
        let mut decoder = nodex_trace::Decoder::default();
        while let Some(Ok(message)) = socket.recv().await {
            let bytes = match message {
                Message::Binary(b) => b,
                Message::Close(_) => break,
                _ => continue,
            };
            match decoder.feed(&bytes) {
                Ok(records) => {
                    if let Ok(mut r) = recorder.lock() {
                        for rec in &records {
                            r.record(rec);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    })
}

#[derive(serde::Serialize)]
pub struct DebugSnapshot {
    status: crate::debugger::DebugStatus,
    stopped: Option<crate::debugger::DebugStopped>,
}

pub async fn debug_snapshot(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(DebugSnapshot { status: state.debug.status(), stopped: state.debug.last_stop() }).into_response())
}

/// Builds the project for debugging and starts it under its debug adapter.
pub async fn debug_start(State(state): State<Arc<AppState>>) -> ApiResult {
    crate::debugger::start(&state).map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(StatusCode::ACCEPTED.into_response())
}

#[derive(Deserialize)]
pub struct CommandRequest {
    command: crate::debugger::DebugCommand,
}

pub async fn debug_command(State(state): State<Arc<AppState>>, Json(req): Json<CommandRequest>) -> ApiResult {
    tokio::task::spawn_blocking(move || state.debug.command(req.command)).await.map_err(internal)?.map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(StatusCode::ACCEPTED.into_response())
}

pub async fn get_breakpoints(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(crate::debugger::breakpoints(&state)).into_response())
}

#[derive(Deserialize)]
pub struct BreakpointsRequest {
    file: String,
    lines: Vec<u32>,
}

/// Replaces the breakpoints of one file.
pub async fn put_breakpoints(State(state): State<Arc<AppState>>, Json(req): Json<BreakpointsRequest>) -> ApiResult {
    let all = crate::debugger::set_breakpoints(&state, &req.file, req.lines).map_err(internal)?;
    Ok(Json(all).into_response())
}

#[derive(Deserialize)]
pub struct VariablesQuery {
    reference: i64,
}

pub async fn debug_variables(State(state): State<Arc<AppState>>, Query(q): Query<VariablesQuery>) -> ApiResult {
    let vars = tokio::task::spawn_blocking(move || state.debug.variables(q.reference)).await.map_err(internal)?.map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(Json(vars).into_response())
}

#[derive(Deserialize)]
pub struct ScopesQuery {
    frame: i64,
}

pub async fn debug_scopes(State(state): State<Arc<AppState>>, Query(q): Query<ScopesQuery>) -> ApiResult {
    let scopes = tokio::task::spawn_blocking(move || state.debug.scopes(q.frame)).await.map_err(internal)?.map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(Json(scopes).into_response())
}

#[derive(Deserialize)]
pub struct EvaluateRequest {
    expression: String,
    frame: Option<i64>,
}

pub async fn debug_evaluate(State(state): State<Arc<AppState>>, Json(req): Json<EvaluateRequest>) -> ApiResult {
    let v = tokio::task::spawn_blocking(move || state.debug.evaluate(&req.expression, req.frame))
        .await
        .map_err(internal)?
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(v).into_response())
}

pub async fn debug_adapters(State(state): State<Arc<AppState>>) -> ApiResult {
    let (frontends, root) = {
        let project = state.project.lock().map_err(poisoned)?;
        (project.active_frontends(), project.root().to_path_buf())
    };
    Ok(Json(crate::adapters::statuses(&frontends, &root)).into_response())
}

#[derive(Deserialize)]
pub struct InstallRequest {
    language: nodex_ir::Language,
}

/// Downloads a debug adapter (the user asked for it); progress arrives over the WebSocket.
pub async fn debug_adapter_install(State(state): State<Arc<AppState>>, Json(req): Json<InstallRequest>) -> ApiResult {
    crate::adapters::start_install(&state, req.language).map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(StatusCode::ACCEPTED.into_response())
}

pub async fn ws(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| client(socket, state))
}

async fn client(mut socket: WebSocket, state: Arc<AppState>) {
    let mut events = state.events.subscribe();
    // A folder opened before this browser connected (the desktop app's command line): say so,
    // or the browser would miss `project-loading`.
    let loading = state.loading.lock().ok().and_then(|l| l.clone());
    if let Some(path) = loading
        && let Ok(text) = serde_json::to_string(&crate::ServerEvent::ProjectLoading { path })
        && socket.send(Message::Text(text.into())).await.is_err()
    {
        return;
    }
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    let Ok(text) = serde_json::to_string(&event) else { continue };
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}

fn poisoned<T>(_: T) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, "project lock poisoned".into())
}

fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn project_root(state: &AppState) -> Result<std::path::PathBuf, (StatusCode, String)> {
    Ok(state.project.lock().map_err(poisoned)?.root().to_path_buf())
}

/// The Unity card: package, Editor, trace probes (`null` for other projects).
pub async fn unity_status(State(state): State<Arc<AppState>>) -> ApiResult {
    let root = project_root(&state)?;
    let status = tokio::task::spawn_blocking(move || crate::unity::status(&root)).await.map_err(internal)?;
    Ok(Json(status).into_response())
}

#[derive(Deserialize)]
pub struct UnityPackageRequest {
    action: crate::unity::PackageAction,
}

/// Adds (Link, Embed), updates or removes the Nodex package in the Unity project.
pub async fn unity_package(State(state): State<Arc<AppState>>, Json(req): Json<UnityPackageRequest>) -> ApiResult {
    let root = project_root(&state)?;
    tokio::task::spawn_blocking(move || crate::unity::package(&root, req.action))
        .await
        .map_err(internal)?
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Scripts without trace probes again.
pub async fn unity_trace_off(State(state): State<Arc<AppState>>) -> ApiResult {
    let root = project_root(&state)?;
    crate::unity::trace_off(&root).map_err(internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
