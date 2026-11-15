use std::fs;
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;

fn mock_cli(dir: &Path) -> PathBuf {
    let source = dir.join("mock.rs");
    fs::write(&source, r##"
use std::io::Read;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.last().is_some_and(|a| a == "--help") {
        println!("--ephemeral --ignore-user-config --ignore-rules --output-schema --json --sandbox");
        return;
    }
    if args.get(1).is_some_and(|a| a == "features") {
        println!("shell_tool stable true\nview_image stable true\napps stable true\nplugins stable true\nhooks stable true");
        return;
    }
    for flag in ["--ephemeral", "--ignore-user-config", "read-only", "shell_tool", "--output-schema"] {
        assert!(args.iter().any(|arg| arg == flag));
    }
    assert!(!std::path::Path::new("Cargo.toml").exists());
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    let dir = std::env::current_exe().unwrap();
    let dir = dir.parent().unwrap();
    std::fs::write(dir.join("received"), text).unwrap();
    if dir.join("hold").exists() {
        std::fs::write(dir.join("running"), "").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    let output = &args[args.iter().position(|a| a == "--output-last-message").unwrap() + 1];
    let json = if dir.join("invalid").exists() { "{}" } else {
        r#"{"suggestions":[{"kind":"summary","target":"","text":"Coordinates purchases.","members":[],"evidence":[]}]}"#
    };
    std::fs::write(output, json).unwrap();
    println!("{{\"type\":\"turn.completed\"}}");
}
"##).unwrap();
    let binary = dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    assert!(
        nodex_lang_api::process::command("rustc")
            .arg(source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    binary
}

fn project(path: &Path) -> nodex_core::Project {
    nodex_core::Project::open(path, vec![Arc::new(nodex_lang_csharp::CSharpFrontend)]).unwrap()
}

async fn request(
    state: &Arc<AppState>,
    method: &str,
    uri: &str,
    data: Option<Value>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri(uri)
        .method(method)
        .header("host", "127.0.0.1:7420")
        .header("x-nodex-token", "session")
        .header("content-type", "application/json")
        .body(data.map_or_else(Body::empty, |v| Body::from(v.to_string())))
        .unwrap();
    let res = crate::router(state.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let data = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&data)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&data).into())),
    )
}

const URI: &str = "/api/diagram-refinements";

async fn wait_for_job(state: &Arc<AppState>, job: &Value) -> Value {
    let url = format!("{URI}/jobs/{}", job.as_str().unwrap());
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let (s, p) = request(state, "GET", &url, None).await;
            assert_eq!(s, StatusCode::OK, "{p}");
            if p["phase"] != "running" {
                return p;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn consent_is_bound_to_the_preview_and_versions_are_local_and_revision_checked() {
    let executable_dir = tempfile::tempdir().unwrap();
    let binary = mock_cli(executable_dir.path());
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::create_dir(root.join("Services")).unwrap();
    fs::write(
        root.join("Services/Workers.cs"),
        "namespace Demo; public class Worker { public void Run() {} }\n// password = SECRET123\n",
    )
    .unwrap();
    fs::write(root.join("README.md"), "# Workshop\nHandles orders.").unwrap();
    fs::write(root.join(".env"), "HIDDEN=secret").unwrap();
    let state = AppState::new(project(root), "session".into(), 7420, vec![]);
    *state.refinements.1.lock().unwrap() = Some(binary.clone());
    let target =
        json!({"kind":"architecture","view":"layers","scope":{"kind":"Project"},"container":null});
    let uri = format!(
        "{URI}?{}",
        form_urlencoded::Serializer::new(String::new())
            .append_pair("target", &target.to_string())
            .finish()
    );
    let (_, history) = request(&state, "GET", &uri, None).await;
    assert!(history["versions"].as_array().unwrap().is_empty());
    let (status, preview) = request(
        &state,
        "POST",
        &format!("{URI}/context"),
        Some(json!({"target":target})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let input = preview["input"].as_str().unwrap();
    assert!(
        input.contains("Handles orders.") && input.contains("[redacted]"),
        "{input}"
    );
    assert!(!input.contains("SECRET123") && !input.contains("HIDDEN"));
    let (status, reduced) = request(
        &state,
        "POST",
        &format!("{URI}/context"),
        Some(json!({
            "target": target, "exclude": [{"path":"README.md", "line":1}]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reduced}");
    assert!(
        !reduced["input"]
            .as_str()
            .unwrap()
            .contains("Handles orders.")
    );

    let start = |digest: &Value, confirmed: bool| json!({"preview":preview["id"],"digest":digest,"confirmed":confirmed});
    let jobs = format!("{URI}/jobs");
    assert_eq!(
        request(
            &state,
            "POST",
            &jobs,
            Some(start(&preview["digest"], false))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &state,
            "POST",
            &jobs,
            Some(start(&json!("different"), true))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(!executable_dir.path().join("received").exists());
    let (status, job) = request(&state, "POST", &jobs, Some(start(&preview["digest"], true))).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    let ready = wait_for_job(&state, &job).await;
    assert_eq!(ready["phase"], "ready", "{ready}");
    assert_eq!(
        fs::read_to_string(executable_dir.path().join("received")).unwrap(),
        input
    );
    assert!(!root.join(DIRECTORY).exists());
    let save = json!({"action":"approve","job":job,"selected":[0],"revision":history["revision"]});
    let (a, b) = tokio::join!(
        request(&state, "PUT", URI, Some(save.clone())),
        request(&state, "PUT", URI, Some(save))
    );
    assert!(
        matches!(
            (a.0, b.0),
            (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
        ),
        "{a:?} {b:?}"
    );
    let (status, saved) = request(&state, "GET", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let saved_id = saved["active"].clone();
    assert_eq!(saved["versions"][0]["basis"], saved["current"]);
    let reopened = AppState::new(project(root), "session".into(), 7420, vec![]);
    let (_, offline) = request(&reopened, "GET", &uri, None).await;
    assert_eq!(offline["active"], saved_id);
    assert_eq!(offline["current"], saved["current"]);

    let mut events = reopened.events.subscribe();
    let watcher = crate::watch::spawn(reopened.clone()).unwrap();
    let before_document_edit = capture(&reopened, serde_json::from_value(target.clone()).unwrap())
        .await
        .unwrap();
    fs::write(root.join("README.md"), "# Workshop\nManages receipts.").unwrap();
    assert!(matches!(
        check_basis(&reopened, &before_document_edit),
        Err((StatusCode::CONFLICT, _))
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            events.recv().await.unwrap(),
            crate::ServerEvent::DiagramEvidenceChanged
        ) {}
    })
    .await
    .unwrap();
    let (_, documented) = request(&reopened, "GET", &uri, None).await;
    assert_ne!(documented["current"], saved["current"]);
    assert_eq!(documented["versions"][0], saved["versions"][0]);
    drop(watcher);

    fs::write(
        root.join("Services/Workers.cs"),
        "namespace Demo; public class Worker { public void Updated() {} }",
    )
    .unwrap();
    let changed = AppState::new(project(root), "session".into(), 7420, vec![]);
    let (_, historical) = request(&changed, "GET", &uri, None).await;
    assert_ne!(historical["current"], saved["current"]);
    assert_eq!(historical["versions"][0], saved["versions"][0]);

    // The mocked CLI can also fail without touching the saved history.
    *changed.refinements.1.lock().unwrap() = Some(binary.clone());
    fs::write(executable_dir.path().join("invalid"), "").unwrap();
    let (_, preview) = request(
        &changed,
        "POST",
        &format!("{URI}/context"),
        Some(json!({"target":target})),
    )
    .await;
    let (_, invalid) = request(
        &changed,
        "POST",
        &jobs,
        Some(json!({"preview":preview["id"],"digest":preview["digest"],"confirmed":true})),
    )
    .await;
    assert_eq!(wait_for_job(&changed, &invalid).await["phase"], "failed");
    fs::write(executable_dir.path().join("hold"), "").unwrap();
    let (_, preview) = request(
        &changed,
        "POST",
        &format!("{URI}/context"),
        Some(json!({"target":target})),
    )
    .await;
    let (_, running) = request(
        &changed,
        "POST",
        &jobs,
        Some(json!({"preview":preview["id"],"digest":preview["digest"],"confirmed":true})),
    )
    .await;
    let job_uri = format!("{jobs}/{}", running.as_str().unwrap());
    tokio::time::timeout(Duration::from_secs(8), async {
        while !executable_dir.path().join("running").exists() {
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        request(&changed, "DELETE", &job_uri, None).await.1["phase"],
        "cancelled"
    );
    changed.close();
    assert_eq!(
        request(&changed, "GET", &job_uri, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn excluding_an_excerpt_rejects_overlapping_replacements() {
    let temp = tempfile::tempdir().unwrap();
    let lines = (1..=100)
        .map(|i| format!("line-{i:03}"))
        .collect::<Vec<_>>();
    fs::write(temp.path().join("example.cs"), lines.join("\n")).unwrap();
    let evidence = |id: &str, line| refinement::Evidence {
        id: id.into(),
        text: "Source reference".into(),
        path: Some("example.cs".into()),
        site: Some(nodex_ir::Span {
            file: "example.cs".into(),
            start_byte: 0,
            end_byte: 0,
            start_line: line,
            end_line: line,
            start_col: 1,
            end_col: 1,
        }),
    };
    let base = Inventory {
        title: "Demo".into(),
        nodes: Vec::new(),
        edges: Vec::new(),
        evidence: vec![evidence("e0", 4), evidence("e1", 20), evidence("e2", 60)],
        candidates: Vec::new(),
        pending: Vec::new(),
        recursive: false,
        layers: false,
        uml: None,
    };
    let original = context::pack(&base, temp.path(), &[], None);
    assert_eq!(original.snippets[0].id, "e0");
    assert!(original.input.contains("line-020"));
    for exclusion in [
        context::ExcludedExcerpt::EvidenceId("e0".into()),
        context::ExcludedExcerpt::Region {
            path: "example.cs".into(),
            line: 1,
        },
    ] {
        let updated = context::pack(&base, temp.path(), &[exclusion], None);
        assert_eq!(updated.snippets[0].id, "e2");
        assert!(!updated.input.contains("line-020"));
        assert!(updated.input.contains("line-060"));
    }
}
