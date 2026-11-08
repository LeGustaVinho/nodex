//! The embeddable server: starting without a folder (the desktop app's welcome screen), opening
//! one, the environment report and shutting down.

use std::path::PathBuf;

use nodex_server::{ServerOptions, start};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Sends one HTTP/1.1 request and returns the status code and body.
async fn http(port: u16, token: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.expect("connects");
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nx-nodex-token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let text = String::from_utf8_lossy(&response).into_owned();
    let status = text.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = text.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
    (status, body)
}

#[tokio::test(flavor = "multi_thread")]
async fn starts_without_a_project_then_opens_one_and_shuts_down() {
    // SAFETY: set before any thread reads the environment in this test process.
    unsafe {
        std::env::set_var("NODEX_NO_SEMANTIC", "1");
        std::env::set_var("NODEX_CONFIG_DIR", std::env::temp_dir().join("nodex-test-config-server"));
    }
    let server = start(ServerOptions::default()).await.expect("starts");
    assert_ne!(server.port, 0, "a real port is reported for port 0");
    let (port, token) = (server.port, server.token.clone());

    let (status, body) = http(port, &token, "GET", "/api/project", None).await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"open\":false"), "{body}");

    // Routes about the folder wait for one.
    let (status, _) = http(port, &token, "GET", "/api/structure", None).await;
    assert_eq!(status, 409);
    // The token is still checked first.
    let (status, _) = http(port, "wrong", "GET", "/api/structure", None).await;
    assert_eq!(status, 401);
    // Routes that do not need a folder answer.
    let (status, _) = http(port, &token, "GET", "/api/catalog", None).await;
    assert_eq!(status, 200);

    let path = serde_json::to_string(&fixtures().join("rust/Sample").to_string_lossy()).unwrap();
    let (status, body) = http(port, &token, "POST", "/api/project/open", Some(&format!("{{\"path\":{path}}}"))).await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"open\":true"), "{body}");
    let (status, body) = http(port, &token, "GET", "/api/structure", None).await;
    assert_eq!(status, 200);
    let graph: serde_json::Value = serde_json::from_str(&body).unwrap();
    let id = graph["symbols"].as_array().unwrap().iter()
        .find(|s| s["kind"] == "Function" && s["origins"].as_array().is_some_and(|o| !o.is_empty()))
        .and_then(|s| s["id"].as_str()).expect("project function");
    let query = |kind: &str| format!("/api/navigation?{}", form_urlencoded::Serializer::new(String::new())
        .append_pair("symbol", id).append_pair("kind", kind).finish());
    let (status, body) = http(port, &token, "GET", &query("declaration"), None).await;
    assert_eq!(status, 200);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["coverage"], "indexed");
    assert_eq!(data["locations"][0]["symbol"], id);
    assert!(data["locations"][0]["span"]["start_line"].as_u64().unwrap() > 0);
    let file = data["locations"][0]["span"]["file"].as_str().unwrap();
    let source_query = form_urlencoded::Serializer::new(String::new()).append_pair("file", file).finish();
    let (status, source) = http(port, &token, "GET", &format!("/api/source?{source_query}"), None).await;
    assert_eq!(status, 200);
    assert_eq!(data["locations"][0]["source_revision"], nodex_core::navigation::source_revision(&source));
    let (status, body) = http(port, &token, "GET", &query("usages"), None).await;
    assert_eq!(status, 200);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["coverage"], "partial");
    assert_eq!(http(port, "wrong", "GET", &query("usages"), None).await.0, 401);
    assert_eq!(http(port, &token, "GET", "/api/navigation?symbol=missing&kind=usages", None).await.0, 404);
    assert!(server.state.watcher.lock().unwrap().is_some());

    // Closing goes back to the welcome screen; opening again works.
    let (status, body) = http(port, &token, "POST", "/api/project/close", None).await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"open\":false"), "{body}");
    assert!(server.state.watcher.lock().unwrap().is_none());
    let (status, _) = http(port, &token, "GET", "/api/structure", None).await;
    assert_eq!(status, 409);
    let (status, body) = http(port, &token, "POST", "/api/project/open", Some(&format!("{{\"path\":{path}}}"))).await;
    assert_eq!(status, 200, "{body}");

    let state = server.state.clone();
    server.shutdown().await;
    assert!(state.watcher.lock().unwrap().is_none(), "the watcher stops");
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_err(), "the port is closed");
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_the_environment() {
    let report = tokio::task::spawn_blocking(nodex_server::environment::report).await.unwrap();
    let ids: Vec<&str> = report.tools.iter().map(|t| t.id.as_str()).collect();
    for id in ["dotnet", "node", "cargo", "rust-analyzer", "unity", "roslyn", "weaver", "nodex-ts", "rust-trace-runtime", "unity-package"] {
        assert!(ids.contains(&id), "{id} in {ids:?}");
    }
    // These tests run under cargo, and the Rust runtime and Unity package are source files.
    assert!(report.tools.iter().any(|t| t.id == "cargo" && t.found));
    assert!(report.tools.iter().any(|t| t.id == "rust-trace-runtime" && t.found));
    assert!(report.tools.iter().any(|t| t.id == "unity-package" && t.found));
    assert!(report.tools.iter().filter(|t| !t.found).all(|t| t.hint.is_some()));
}
