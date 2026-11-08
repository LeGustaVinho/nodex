//! End-to-end debugging with the real adapters (downloaded on first use): break on the line that
//! adds to `total` in each Sample, read the stack and variables, step, continue, stop.
//!
//! `cargo test -p nodex-server --test debug -- --ignored --test-threads 1`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_server::debugger::{self, DebugCommand, DebugState, DebugStopped};
use nodex_server::{AppState, ServerEvent, adapters};
use tokio::sync::broadcast::Receiver;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

async fn next_stop(events: &mut Receiver<ServerEvent>, output: &mut Vec<String>) -> DebugStopped {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.unwrap_or_else(|_| panic!("no stop in time:\n{}", output.join("\n"))).expect("event");
        match event {
            ServerEvent::DebugStopped(s) => return *s,
            ServerEvent::RunOutput { lines } => output.extend(lines.into_iter().map(|l| l.text)),
            ServerEvent::DebugStatus(s) if matches!(s.state, DebugState::Failed | DebugState::Exited) => panic!("{s:?}\n{}", output.join("\n")),
            _ => {}
        }
    }
}

async fn debug_sample(frontend: Arc<dyn LanguageFrontend>, rel: &str, file: &str, line: u32, flow_suffix: &str) {
    // SAFETY: set before the server's threads read the environment.
    unsafe {
        std::env::set_var("NODEX_NO_SEMANTIC", "1");
    }
    let spec = frontend.debugger(Path::new(".")).expect("debugger").adapter();
    if adapters::locate(&spec).is_none() {
        let path = tokio::task::spawn_blocking(move || adapters::install(&spec, &mut |_, _| {})).await.unwrap().expect("adapter installs");
        assert!(path.exists());
    }
    let project = Project::open(&fixtures().join(rel), vec![frontend]).unwrap();
    let state = AppState::new(project, "t".into(), 7400, Vec::new());
    debugger::set_breakpoints(&state, file, vec![line]).unwrap();
    let mut events = state.events.subscribe();
    let mut output = Vec::new();

    debugger::start(&state).expect("debugging starts");
    // A failing assertion must not leave the adapter and the program running.
    struct StopOnPanic(Arc<AppState>);
    impl Drop for StopOnPanic {
        fn drop(&mut self) {
            let _ = self.0.debug.command(DebugCommand::Stop);
            std::thread::sleep(Duration::from_millis(1500));
        }
    }
    let _guard = StopOnPanic(state.clone());
    let stop = next_stop(&mut events, &mut output).await;
    let top = stop.frames.iter().find(|f| f.file.is_some()).unwrap_or_else(|| panic!("{stop:?}"));
    assert_eq!(top.file.as_deref(), Some(file), "{stop:?}");
    assert_eq!(top.line, line as i64, "{stop:?}");
    assert!(top.flow.as_deref().is_some_and(|f| f.ends_with(flow_suffix)), "{stop:?}");
    assert!(top.node.is_some(), "the line maps to a Blueprint node: {stop:?}");
    let vars: Vec<_> = stop.scopes.iter().flat_map(|s| s.variables.iter()).collect();
    assert!(vars.iter().any(|v| v.name == "total"), "{vars:?}");

    // Step over the addition: `total` changed.
    let s2 = state.clone();
    tokio::task::spawn_blocking(move || s2.debug.command(DebugCommand::Next)).await.unwrap().unwrap();
    let stepped = next_stop(&mut events, &mut output).await;
    assert!(stepped.frames.iter().any(|f| f.file.as_deref() == Some(file)), "{stepped:?}");

    // Continue: the breakpoint is hit again for alice's second order.
    let s2 = state.clone();
    tokio::task::spawn_blocking(move || s2.debug.command(DebugCommand::Continue)).await.unwrap().unwrap();
    let again = next_stop(&mut events, &mut output).await;
    let top = again.frames.iter().find(|f| f.file.is_some()).unwrap();
    assert_eq!(top.line, line as i64, "{again:?}");

    let s2 = state.clone();
    tokio::task::spawn_blocking(move || s2.debug.command(DebugCommand::Stop)).await.unwrap().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("stops in time").expect("event");
        if let ServerEvent::DebugStatus(s) = event
            && matches!(s.state, DebugState::Exited | DebugState::Failed)
        {
            assert_eq!(s.state, DebugState::Exited, "{s:?}");
            break;
        }
    }
    assert!(!state.debug.is_busy());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads netcoredbg; needs the .NET SDK"]
async fn debugs_the_csharp_sample() {
    debug_sample(Arc::new(nodex_lang_csharp::CSharpFrontend), "csharp/Sample", "Services/OrderService.cs", 66, "TotalForCustomer(string)").await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads vscode-js-debug; needs Node.js and the nodex-ts sidecar"]
async fn debugs_the_typescript_sample() {
    debug_sample(Arc::new(nodex_lang_typescript::TypeScriptFrontend), "ts/Sample", "src/services/orderService.ts", 20, "totalForCustomer").await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads CodeLLDB; needs cargo"]
async fn debugs_the_rust_sample() {
    debug_sample(Arc::new(nodex_lang_rust::RustFrontend), "rust/Sample", "src/services/order_service.rs", 54, "total_for_customer").await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Node.js, the nodex-ts sidecar and vscode-js-debug"]
async fn stop_during_the_build_ends_without_launching() {
    // SAFETY: set before the server's threads read the environment.
    unsafe {
        std::env::set_var("NODEX_NO_SEMANTIC", "1");
    }
    let frontend: Arc<dyn LanguageFrontend> = Arc::new(nodex_lang_typescript::TypeScriptFrontend);
    let spec = frontend.debugger(Path::new(".")).expect("debugger").adapter();
    if adapters::locate(&spec).is_none() {
        tokio::task::spawn_blocking(move || adapters::install(&spec, &mut |_, _| {})).await.unwrap().expect("adapter installs");
    }
    let project = Project::open(&fixtures().join("ts/Sample"), vec![frontend]).unwrap();
    let state = AppState::new(project, "t".into(), 7401, Vec::new());
    let mut events = state.events.subscribe();
    debugger::start(&state).expect("debugging starts");
    // Busy from the first moment: a Run or a second session is refused while it builds.
    assert!(state.debug.is_busy());
    assert!(debugger::start(&state).is_err());
    assert!(nodex_server::runner::start(&state).is_err());
    // Only Stop is accepted before the program runs.
    assert!(state.debug.command(DebugCommand::Next).is_err());
    state.debug.command(DebugCommand::Stop).expect("stop during the build");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("ends in time").expect("event");
        match event {
            ServerEvent::DebugStatus(s) if s.state == DebugState::Running => panic!("launched after Stop: {s:?}"),
            ServerEvent::DebugStatus(s) if matches!(s.state, DebugState::Exited | DebugState::Failed) => {
                assert_eq!(s.state, DebugState::Exited, "{s:?}");
                break;
            }
            _ => {}
        }
    }
    assert!(!state.debug.is_busy());
}
