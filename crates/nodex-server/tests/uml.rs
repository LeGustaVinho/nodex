//! UML diagrams over the API: scopes, errors, cache invalidation after an edit.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_server::{AppState, router};
use tower::ServiceExt;

const TOKEN: &str = "t";

fn state() -> Arc<AppState> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/csharp/Sample");
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_csharp::CSharpFrontend)];
    AppState::new(Project::open(&root, frontends).unwrap(), TOKEN.into(), 7420, Vec::new())
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

async fn get(state: &Arc<AppState>, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder().uri(uri).header("host", "127.0.0.1:7420").header("x-nodex-token", TOKEN).body(Body::empty()).unwrap();
    let res = router(state.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn serves_diagrams_for_every_scope() {
    let state = state();
    let (status, d) = get(&state, &format!("/api/uml?kind=Class&scope={}", enc(r#"{"kind":"Project"}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(d["mermaid"].as_str().unwrap().starts_with("classDiagram"));
    assert!(d["elements"].as_array().unwrap().iter().any(|e| e["label"] == "OrderService"));

    let scope = enc(r#"{"kind":"Folder","path":"Services"}"#);
    let opts = enc(r#"{"members":"None","context_neighbors":false}"#);
    let (status, d) = get(&state, &format!("/api/uml?kind=Package&scope={scope}&opts={opts}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(d["mermaid"].as_str().unwrap().starts_with("flowchart"));
    assert_eq!(d["title"], "Folder Services");

    let (status, a) = get(&state, &format!("/api/uml/availability?scope={}", enc(r#"{"kind":"File","path":"Services/OrderService.cs"}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(a["suggested"], "Class");
}

#[tokio::test]
async fn reads_function_bodies_for_activity_and_sequence_diagrams() {
    let state = state();
    let scope = enc(r#"{"kind":"Symbol","id":"Sample.Services.OrderService.ShipAsync(int)"}"#);
    let (status, d) = get(&state, &format!("/api/uml?kind=Activity&scope={scope}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(d["mermaid"].as_str().unwrap().starts_with("flowchart TD"));
    assert_eq!(d["title"], "Activity of OrderService.ShipAsync");
    assert!(d["elements"].as_array().unwrap().iter().any(|e| e["kind"] == "decision"));

    // A broad scope offers its functions; the focus option picks one.
    let (status, d) = get(&state, &format!("/api/uml?kind=Sequence&scope={}", enc(r#"{"kind":"Project"}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(d["mermaid"].as_str().unwrap().starts_with("sequenceDiagram"));
    let candidates = d["candidates"].as_array().unwrap();
    assert!(candidates.len() > 3);
    let other = candidates[1]["id"].as_str().unwrap().to_owned();
    let opts = enc(&serde_json::json!({ "focus": other }).to_string());
    let (_, focused) = get(&state, &format!("/api/uml?kind=Sequence&scope={}&opts={opts}", enc(r#"{"kind":"Project"}"#))).await;
    assert_ne!(focused["title"], d["title"]);
}

#[tokio::test]
async fn reports_bad_scopes_and_unknown_symbols() {
    let state = state();
    let (status, _) = get(&state, "/api/uml?kind=Class&scope=nope").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(&state, &format!("/api/uml?kind=Class&scope={}", enc(r#"{"kind":"Symbol","id":"No.Such.Type"}"#))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A recorded sequence before anything ran: a hint, not an error.
    let opts = enc(r#"{"runtime":true}"#);
    let (status, d) = get(&state, &format!("/api/uml?kind=Sequence&scope={}&opts={opts}", enc(r#"{"kind":"Project"}"#))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(d["warnings"][0].as_str().unwrap().contains("No recorded calls"));
}

#[tokio::test]
async fn cached_diagrams_are_rebuilt_when_the_graph_changes() {
    let state = state();
    let uri = format!("/api/uml?kind=Class&scope={}", enc(r#"{"kind":"Project"}"#));
    let (_, first) = get(&state, &uri).await;
    let (_, again) = get(&state, &uri).await;
    assert_eq!(first, again, "served from the cache");
    // Semantic facts arriving invalidate the graph: the next request builds a new diagram.
    state.project.lock().unwrap().apply_semantic(nodex_ir::Language::Csharp, nodex_ir::semantic::SemanticFacts::default());
    let (status, rebuilt) = get(&state, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rebuilt["kind"], "Class");
}
