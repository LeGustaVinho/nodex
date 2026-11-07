//! Serves the built web UI (`web/dist`), embedded into the binary in release builds.

use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Assets;

const NOT_BUILT: &str = "<!doctype html><meta charset=utf-8><title>Nodex</title>\
<body style=\"font-family:system-ui;background:#1b1d22;color:#e6e6e6;padding:2rem\">\
<h1>Nodex web UI is not built</h1>\
<p>Run <code>pnpm --dir web install</code> and <code>pnpm --dir web build</code>, then restart the server, \
or start the Vite dev server with <code>pnpm --dir web dev</code>.</p>";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
        return ([(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, cache)], file.data.into_owned()).into_response();
    }
    // Single-page app: unknown paths without an extension get index.html.
    if !path.contains('.') || path == "index.html" {
        return match Assets::get("index.html") {
            Some(index) => ([(header::CACHE_CONTROL, "no-cache")], Html(index.data.into_owned())).into_response(),
            None => Html(NOT_BUILT).into_response(),
        };
    }
    StatusCode::NOT_FOUND.into_response()
}
