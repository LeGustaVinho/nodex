//! Unity end to end, in a batch-mode Unity Editor on `fixtures/unity-sample`: the Nodex package
//! is linked for the test (and the manifest restored after), then
//! - a live trace of Play Mode records `OrderLoop.TotalForCustomer`;
//! - the debugger attaches to the Editor, stops on `total += order.Amount;` and detaches,
//!   leaving the Editor running.
//!
//! Needs Unity 6000.3.20f1 (or `NODEX_UNITY_EDITOR`) and a license:
//! `cargo test -p nodex-server --test unity -- --ignored --test-threads 1`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nodex_core::Project;
use nodex_lang_csharp::unity_editor;
use nodex_lang_api::LanguageFrontend;
use nodex_server::debugger::{self, DebugCommand, DebugState};
use nodex_server::runner::RunState;
use nodex_server::{AppState, ServerEvent, adapters};

/// The sample's absolute path without the Windows `\\?\` prefix (Unity cannot open those).
fn sample() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/unity-sample").canonicalize().unwrap();
    let text = path.to_string_lossy().into_owned();
    PathBuf::from(text.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(text))
}

fn unity_exe() -> PathBuf {
    std::env::var_os("NODEX_UNITY_EDITOR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"E:\Unity\6000.3.20f1\Editor\Unity.exe"))
}

/// A batch-mode Editor on the sample, with the package linked; everything is undone on drop
/// (even when an assertion fails).
struct Editor {
    root: PathBuf,
    manifest: String,
    child: std::process::Child,
}

impl Editor {
    fn start(root: &Path, extra: &[&str]) -> Editor {
        let manifest = std::fs::read_to_string(root.join("Packages/manifest.json")).unwrap();
        unity_editor::link_package(root).expect("package links");
        let _ = std::fs::remove_file(root.join(".nodex/unity/session.json"));
        let _ = std::fs::remove_file(root.join(".nodex/unity/editor.json"));
        let log = std::env::temp_dir().join(format!("nodex-unity-{}.log", std::process::id()));
        let child = std::process::Command::new(unity_exe())
            .args(["-batchmode", "-nographics", "-projectPath"])
            .arg(root)
            .arg("-logFile")
            .arg(&log)
            .args(["-executeMethod", "NodexFixture.Stay"])
            .args(extra)
            .env("NODEX_FIXTURE_SECONDS", "420")
            .spawn()
            .expect("Unity starts");
        Editor { root: root.to_path_buf(), manifest, child }
    }

    /// Waits for the Editor and its Nodex bridge (the first import compiles the package).
    async fn ready(&self, what: impl Fn(&unity_editor::BridgeState) -> bool) -> unity_editor::BridgeState {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        loop {
            if let Some(b) = unity_editor::bridge(&self.root)
                && unity_editor::editor(&self.root).is_some()
                && what(&b)
            {
                return b;
            }
            assert!(tokio::time::Instant::now() < deadline, "the Unity Editor was not ready in time");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = std::process::Command::new("taskkill").args(["/PID", &self.child.id().to_string(), "/T", "/F"]).output();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::write(self.root.join("Packages/manifest.json"), &self.manifest);
        // A killed Editor leaves scene backups; the next Editor would ask to recover them.
        let _ = std::fs::remove_dir_all(self.root.join("Temp"));
        let _ = std::fs::remove_file(self.root.join(".nodex/unity/session.json"));
    }
}

fn state(root: &Path) -> Arc<AppState> {
    // SAFETY: set before the server's threads read the environment.
    unsafe {
        std::env::set_var("NODEX_NO_SEMANTIC", "1");
    }
    let project = Project::open(root, vec![Arc::new(nodex_lang_csharp::CSharpFrontend) as Arc<dyn LanguageFrontend>]).unwrap();
    AppState::new(project, "t".into(), 7402, Vec::new())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "runs a batch-mode Unity Editor"]
async fn traces_unity_play_mode() {
    let root = sample();
    unity_editor::set_trace(&root, true).unwrap();
    let editor = Editor::start(&root, &[]);
    editor.ready(|b| b.trace_compiled && !b.compiling).await;
    let state = state(&root);
    nodex_server::runner::start(&state).expect("run starts");

    // Run asks for Play Mode; Update sums alice's orders every frame.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let flow = loop {
        let snap = state.run.snapshot();
        if let Some(trace) = &snap.trace
            && let Some((flow, nodes)) = trace.counts.iter().find(|(f, _)| f.ends_with("TotalForCustomer(string)"))
            && nodes.values().any(|c| *c > 10)
        {
            break flow.clone();
        }
        assert!(tokio::time::Instant::now() < deadline, "no Play Mode trace in time: {:?}\n{:?}", snap.status, snap.output.iter().map(|l| &l.text).collect::<Vec<_>>());
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    let snap = state.run.snapshot();
    let trace = snap.trace.unwrap();
    assert!(trace.calls.keys().any(|f| f.ends_with("Update()")), "{:?}", trace.calls);
    let steps = state.run.recorder().unwrap().lock().unwrap().steps(&flow, 0, 100);
    // A live snapshot can already be inside the next call, whose total starts at zero.
    assert!(steps.steps.iter().any(|step| {
        step.values.iter().any(|value| value == "total = 195")
            && step.observations.iter().flatten().any(|observation| {
                observation.name == "total" && observation.shape == nodex_ir::types::TypeShape::scalar("Int32")
            })
    }), "{steps:?}");

    // Stop leaves Play Mode; the Editor keeps running.
    assert!(state.run.stop());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while state.run.snapshot().status.state != RunState::Exited {
        assert!(tokio::time::Instant::now() < deadline, "the run did not stop");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    editor.ready(|b| !b.playing).await;
    unity_editor::set_trace(&root, false).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "runs a batch-mode Unity Editor and downloads unity-dap"]
async fn debugs_the_unity_editor() {
    let root = sample();
    let file = "Assets/Scripts/OrderLoop.cs";
    let text = std::fs::read_to_string(root.join(file)).unwrap();
    let line = text.lines().position(|l| l.contains("total += order.Amount;")).unwrap() as u32 + 1;

    let frontend = nodex_lang_csharp::CSharpFrontend;
    let spec = frontend.debugger(&root).expect("debugger").adapter();
    assert_eq!(spec.name, "unity-dap");
    if adapters::locate(&spec).is_none() {
        tokio::task::spawn_blocking(move || adapters::install(&spec, &mut |_, _| {})).await.unwrap().expect("adapter installs");
    }
    let editor = Editor::start(&root, &[]);
    editor.ready(|b| !b.compiling).await;
    let state = state(&root);
    debugger::set_breakpoints(&state, file, vec![line]).unwrap();
    let mut events = state.events.subscribe();
    debugger::start(&state).expect("debugging starts");
    // A failing assertion must not leave the session attached.
    struct StopOnPanic(Arc<AppState>);
    impl Drop for StopOnPanic {
        fn drop(&mut self) {
            let _ = self.0.debug.command(DebugCommand::Stop);
            std::thread::sleep(Duration::from_millis(1500));
        }
    }
    let _guard = StopOnPanic(state.clone());

    // Attached (the bridge switched the Editor to Debug mode): now press Play.
    let mut output = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let mut played = false;
    let stop = loop {
        if !played && state.debug.status().state == DebugState::Running {
            let mut s = unity_editor::read_session(&root).unwrap_or_default();
            s.id = "debug-play".into();
            s.play = true;
            unity_editor::write_session(&root, Some(&s)).unwrap();
            played = true;
        }
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Ok(ServerEvent::DebugStopped(s))) => break *s,
            Ok(Ok(ServerEvent::RunOutput { lines })) => output.extend(lines.into_iter().map(|l| l.text)),
            Ok(Ok(ServerEvent::DebugStatus(s))) if matches!(s.state, DebugState::Failed | DebugState::Exited) => panic!("{s:?}\n{}", output.join("\n")),
            _ => {}
        }
        assert!(tokio::time::Instant::now() < deadline, "no stop in time:\n{}", output.join("\n"));
    };
    let top = stop.frames.iter().find(|f| f.file.is_some()).unwrap_or_else(|| panic!("{stop:?}"));
    assert_eq!(top.file.as_deref(), Some(file), "{stop:?}");
    assert_eq!(top.line, line as i64, "{stop:?}");
    assert!(top.flow.as_deref().is_some_and(|f| f.ends_with("TotalForCustomer(string)")), "{stop:?}");
    assert!(top.node.is_some(), "the line maps to a Blueprint node: {stop:?}");
    let vars: Vec<_> = stop.scopes.iter().flat_map(|s| s.variables.iter()).collect();
    assert!(vars.iter().any(|v| v.name == "total"), "{vars:?}");

    // Detach: the session ends, the Editor does not.
    let s2 = state.clone();
    tokio::task::spawn_blocking(move || s2.debug.command(DebugCommand::Stop)).await.unwrap().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while state.debug.is_busy() {
        assert!(tokio::time::Instant::now() < deadline, "the session did not end");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(unity_editor::editor(&root).is_some(), "Stop must not end the Unity Editor");
}
