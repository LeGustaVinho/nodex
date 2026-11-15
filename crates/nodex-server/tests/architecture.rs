use std::fs;
use std::future::{Future, poll_fn};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::Poll;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nodex_core::Project;
use nodex_server::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

fn project(root: &Path) -> Project {
    fs::create_dir_all(root.join("Services")).unwrap();
    fs::write(
        root.join("Services/Work.cs"),
        "namespace Demo; public class Work { public void Run() {} }",
    )
    .unwrap();
    Project::open(root, vec![Arc::new(nodex_lang_csharp::CSharpFrontend)]).unwrap()
}

fn state(root: &Path) -> Arc<AppState> {
    AppState::new(project(root), "session".into(), 7420, Vec::new())
}

async fn request(state: &Arc<AppState>, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let method = if body.is_some() { "PUT" } else { "GET" };
    let request = Request::builder()
        .uri(uri)
        .method(method)
        .header("host", "127.0.0.1:7420")
        .header("x-nodex-token", "session")
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .unwrap();
    let res = router(state.clone()).oneshot(request).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())),
    )
}

const CONFIG: &str = "/api/architecture/config";
const LAYERS: &str = "/api/architecture?view=layers";
const SETTINGS_FILE: &str = ".nodex/architecture.json";

#[tokio::test]
async fn saves_and_restores_local_adjustments_and_invalidates_the_diagram() {
    let dir = tempfile::tempdir().unwrap();
    let app = state(dir.path());
    let (status, inferred) = request(&app, LAYERS, None).await;
    assert_eq!(status, StatusCode::OK, "{inferred}");
    assert_eq!(inferred["modules"][0]["layer"], "Application");
    let (_, mut settings) = request(&app, CONFIG, None).await;
    settings["config"]["layers"] = json!({ "Services": "Domain" });
    let (status, saved) = request(&app, CONFIG, Some(settings.clone())).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        request(&app, CONFIG, Some(settings)).await.0,
        StatusCode::CONFLICT
    );
    let (_, d) = request(&app, LAYERS, None).await;
    assert_eq!(d["modules"][0]["layer"], "Domain");
    let reopened = state(dir.path());
    let (_, d) = request(&reopened, LAYERS, None).await;
    assert_eq!(d["modules"][0]["provenance"], "Declared");
    assert_eq!(d["modules"][0]["layer"], "Domain");
}

#[tokio::test]
async fn concurrent_servers_cannot_both_save_the_same_revision() {
    let dir = tempfile::tempdir().unwrap();
    let first = state(dir.path());
    let second = state(dir.path());
    let (_, mut a) = request(&first, CONFIG, None).await;
    let (_, mut b) = request(&second, CONFIG, None).await;
    a["config"]["system_name"] = json!("First");
    b["config"]["system_name"] = json!("Second");
    let (a, b) = tokio::join!(
        request(&first, CONFIG, Some(a)),
        request(&second, CONFIG, Some(b)),
    );
    let saved = match (a.0, b.0) {
        (StatusCode::OK, StatusCode::CONFLICT) => a.1,
        (StatusCode::CONFLICT, StatusCode::OK) => b.1,
        _ => panic!("expected one successful save, got {a:?} and {b:?}"),
    };
    assert_eq!(request(&first, CONFIG, None).await.1, saved);
}

#[tokio::test]
async fn malformed_config_and_failed_save_keep_the_inferred_diagram_available() {
    let dir = tempfile::tempdir().unwrap();
    let app = state(dir.path());
    fs::create_dir(dir.path().join(".nodex")).unwrap();
    fs::write(dir.path().join(SETTINGS_FILE), "broken").unwrap();
    let (status, diagram) = request(&app, LAYERS, None).await;
    assert_eq!(status, StatusCode::OK, "{diagram}");
    assert!(diagram["warnings"][0].as_str().unwrap().contains("invalid"));
    let (_, mut settings) = request(&app, CONFIG, None).await;
    settings["config"]["layers"] = json!({ "../outside": "Domain" });
    assert_eq!(
        request(&app, CONFIG, Some(settings)).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(SETTINGS_FILE)).unwrap(),
        "broken"
    );

    fs::remove_file(dir.path().join(SETTINGS_FILE)).unwrap();
    fs::remove_dir(dir.path().join(".nodex")).unwrap();
    fs::write(dir.path().join(".nodex"), "blocks directory creation").unwrap();
    let (status, mut settings) = request(&app, CONFIG, None).await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    settings["config"]["layers"] = json!({ "Services": "Domain" });
    assert_eq!(
        request(&app, CONFIG, Some(settings)).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let (status, diagram) = request(&app, LAYERS, None).await;
    assert_eq!(status, StatusCode::OK, "{diagram}");
    assert_eq!(diagram["modules"][0]["layer"], "Application");
}

#[tokio::test]
async fn a_stale_revision_cannot_save_into_the_next_project() {
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let app = state(old.path());
    let (_, mut settings) = request(&app, CONFIG, None).await;
    settings["config"]["system_name"] = json!("The old system");
    let replacement = project(new.path());
    app.generation.fetch_add(1, Ordering::SeqCst);
    *app.project.lock().unwrap() = replacement;
    assert_eq!(
        request(&app, CONFIG, Some(settings)).await.0,
        StatusCode::CONFLICT
    );
    assert!(!new.path().join(SETTINGS_FILE).exists());
    assert_eq!(
        request(
            &app,
            "/api/architecture?view=components&container=gone",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_in_flight_diagram_cannot_return_data_from_the_next_project() {
    let old = tempfile::tempdir().unwrap();
    let new = tempfile::tempdir().unwrap();
    let app = state(old.path());
    let gate = app.build_lock.lock().await;
    let mut pending = Box::pin(request(&app, LAYERS, None));
    poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    *app.project.lock().unwrap() = project(new.path());
    app.generation.fetch_add(1, Ordering::SeqCst);
    drop(gate);
    assert_eq!(pending.await.0, StatusCode::CONFLICT);
    assert_eq!(request(&app, LAYERS, None).await.0, StatusCode::OK);
}
