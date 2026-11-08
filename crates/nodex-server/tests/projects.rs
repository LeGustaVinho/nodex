//! Opening another project while the server runs.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_server::{AppState, ServerEvent, projects};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

#[tokio::test(flavor = "multi_thread")]
async fn opens_another_project_and_restarts_what_watches_it() {
    // SAFETY: set before any thread reads the environment in this test process.
    unsafe {
        std::env::set_var("NODEX_NO_SEMANTIC", "1");
        std::env::set_var("NODEX_CONFIG_DIR", std::env::temp_dir().join("nodex-test-config"));
    }
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![
        Arc::new(nodex_lang_csharp::CSharpFrontend),
        Arc::new(nodex_lang_typescript::TypeScriptFrontend),
        Arc::new(nodex_lang_rust::RustFrontend),
    ];
    let project = Project::open(&fixtures().join("ts/Sample"), frontends).unwrap();
    let state = AppState::new(project, "t".into(), 7400, Vec::new());
    let mut events = state.events.subscribe();

    projects::open(&state, fixtures().join("rust/Sample").to_string_lossy().into_owned()).await.expect("opens");
    assert_eq!(state.generation.load(Ordering::SeqCst), 1);
    let summary = state.project.lock().unwrap().summary();
    assert!(summary.root.ends_with("Sample") && summary.root.contains("rust"), "{}", summary.root);
    assert!(state.watcher.lock().unwrap().is_some());
    assert!(state.layout_file().to_string_lossy().contains("rust"));
    let mut seen = Vec::new();
    while let Ok(e) = events.try_recv() {
        seen.push(e);
    }
    assert!(matches!(seen.first(), Some(ServerEvent::ProjectLoading { .. })), "{seen:?}");
    assert!(seen.iter().any(|e| matches!(e, ServerEvent::ProjectChanged)), "{seen:?}");
    assert!(projects::recent().iter().any(|r| r.path.contains("rust")));

    // A folder that does not exist keeps the current project.
    assert!(projects::open(&state, "Z:/does/not/exist".into()).await.is_err());
    assert_eq!(state.generation.load(Ordering::SeqCst), 1);
}
