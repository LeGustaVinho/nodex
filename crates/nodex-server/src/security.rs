//! Request guards: allowed `Host`/`Origin` and the session token.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Header carrying the session token. WebSocket upgrades use the `token` query parameter instead.
pub const TOKEN_HEADER: &str = "x-nodex-token";

fn allowed_hosts(port: u16) -> [String; 3] {
    [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("[::1]:{port}")]
}

/// Rejects requests whose `Host` is not a loopback name for our port (DNS rebinding) and
/// cross-origin requests from pages other than the Nodex UI.
pub async fn check_host_and_origin(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    if !allowed_hosts(state.port).iter().any(|h| h == host) {
        return (StatusCode::FORBIDDEN, "forbidden host").into_response();
    }
    // Traced browser pages come from the app's dev server; `/trace` checks the run token.
    let trace_intake = req.uri().path() == "/trace";
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()).filter(|_| !trace_intake) {
        let ok = allowed_hosts(state.port).iter().any(|h| origin == format!("http://{h}"))
            || state.dev_origins.iter().any(|d| d == origin);
        if !ok {
            return (StatusCode::FORBIDDEN, "forbidden origin").into_response();
        }
    }
    next.run(req).await
}

/// Requires the session token on API and WebSocket routes.
pub async fn require_token(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let provided = token_from(req.headers(), req.uri().query());
    match provided {
        Some(t) if constant_time_eq(t.as_bytes(), state.token.as_bytes()) => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "missing or invalid token").into_response(),
    }
}

/// Answers 409 while no folder is open (the welcome screen).
pub async fn require_project(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let open = state.project.lock().map(|p| p.is_open()).unwrap_or(false);
    if !open {
        return (StatusCode::CONFLICT, "no project is open").into_response();
    }
    next.run(req).await
}

fn token_from(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(t) = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok()) {
        return Some(t.to_owned());
    }
    query?
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "token")
        .map(|(_, v)| v.to_owned())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
