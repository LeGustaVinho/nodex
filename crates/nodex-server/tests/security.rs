//! The API must refuse foreign hosts, foreign origins and missing tokens.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_server::{AppState, router};
use tower::ServiceExt;

const PORT: u16 = 7420;
const TOKEN: &str = "test-token";

fn app() -> axum::Router {
    app_with_fixture("csharp/Sample")
}

fn app_with_fixture(fixture: &str) -> axum::Router {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(fixture);
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_csharp::CSharpFrontend)];
    let project = Project::open(&root, frontends).unwrap();
    router(AppState::new(project, TOKEN.into(), PORT, vec!["http://localhost:5173".into()]))
}

fn request(uri: &str, host: &str) -> axum::http::request::Builder {
    Request::builder().uri(uri).header("host", host)
}

async fn status(req: Request<Body>) -> StatusCode {
    app().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn rejects_foreign_host() {
    let req = request("/api/project", "evil.example:7420").header("x-nodex-token", TOKEN).body(Body::empty()).unwrap();
    assert_eq!(status(req).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rejects_foreign_origin() {
    let req = request("/api/project", "127.0.0.1:7420")
        .header("x-nodex-token", TOKEN)
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(req).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rejects_missing_or_wrong_token() {
    let req = request("/api/project", "127.0.0.1:7420").body(Body::empty()).unwrap();
    assert_eq!(status(req).await, StatusCode::UNAUTHORIZED);
    let req = request("/api/project", "127.0.0.1:7420").header("x-nodex-token", "nope").body(Body::empty()).unwrap();
    assert_eq!(status(req).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_token_from_header_query_and_dev_origin() {
    let req = request("/api/project", "localhost:7420").header("x-nodex-token", TOKEN).body(Body::empty()).unwrap();
    assert_eq!(status(req).await, StatusCode::OK);
    let req = request(&format!("/api/catalog?token={TOKEN}"), "127.0.0.1:7420")
        .header("origin", "http://localhost:5173")
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(req).await, StatusCode::OK);
}

#[tokio::test]
async fn source_refuses_paths_outside_the_project() {
    for file in ["../../Cargo.toml", "Sample.csproj", "C:/Windows/win.ini"] {
        let uri = format!("/api/source?file={file}&token={TOKEN}");
        let req = request(&uri, "127.0.0.1:7420").body(Body::empty()).unwrap();
        assert_eq!(status(req).await, StatusCode::NOT_FOUND, "{file}");
    }
}

#[tokio::test]
async fn serves_flow_and_structure() {
    let req = request("/api/flow?symbol=Sample.Services.OrderService.TotalForCustomer(string)", "127.0.0.1:7420")
        .header("x-nodex-token", TOKEN)
        .body(Body::empty())
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let flow: nodex_ir::FlowGraph = serde_json::from_slice(&body).unwrap();
    assert!(flow.nodes.iter().any(|n| n.concept == nodex_ir::Concept::LoopForEach));
}

#[tokio::test]
async fn serves_a_batch_of_flows_and_skips_symbols_without_bodies() {
    let req = request(
        "/api/flows?symbols=Shop.Services.OrderService.PlaceOrderAsync(Order%2C%20int),Shop.Services.OrderService,Shop.Services.OrderService.Audit(Order%2C%20string)",
        "127.0.0.1:7420",
    )
    .header("x-nodex-token", TOKEN)
    .body(Body::empty())
    .unwrap();
    let res = app_with_fixture("knowledge/fullstack").oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let flows: Vec<nodex_ir::FlowGraph> = serde_json::from_slice(&body).unwrap();
    assert_eq!(flows.iter().map(|f| f.symbol.as_str()).collect::<Vec<_>>(), [
        "Shop.Services.OrderService.PlaceOrderAsync(Order, int)",
        "Shop.Services.OrderService.Audit(Order, string)",
    ]);
    let increment = flows[0].nodes.iter().find(|n| n.writes.iter().any(|w| w == "_placedCount")).unwrap();
    assert!(increment.flavors.iter().any(|flavor| flavor == "implicit-this"));
}
