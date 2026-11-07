//! REST and WebSocket handlers.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
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
    let graph = state.project.lock().map_err(poisoned)?.structure();
    Ok(Json(&*graph).into_response())
}

#[derive(Deserialize)]
pub struct FlowQuery {
    symbol: String,
}

pub async fn flow(State(state): State<Arc<AppState>>, Query(q): Query<FlowQuery>) -> ApiResult {
    let project = state.project.lock().map_err(poisoned)?;
    let flow = project.flow(&q.symbol).ok_or_else(|| not_found("flow"))?;
    Ok(Json(flow).into_response())
}

#[derive(Deserialize)]
pub struct SourceQuery {
    file: String,
}

pub async fn source(State(state): State<Arc<AppState>>, Query(q): Query<SourceQuery>) -> ApiResult {
    let text = state.project.lock().map_err(poisoned)?.source(&q.file).ok_or_else(|| not_found("file"))?;
    Ok(([("content-type", "text/plain; charset=utf-8")], text).into_response())
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
    if let Some(dir) = state.layout_file.parent() {
        std::fs::create_dir_all(dir).map_err(internal)?;
    }
    let text = serde_json::to_string_pretty(&layouts).map_err(internal)?;
    std::fs::write(&state.layout_file, text).map_err(internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize, serde::Serialize)]
pub struct Position {
    x: f64,
    y: f64,
}

pub async fn ws(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| client(socket, state))
}

async fn client(mut socket: WebSocket, state: Arc<AppState>) {
    let mut events = state.events.subscribe();
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
