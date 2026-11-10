//! End-to-end live trace: build, weave, run the C# sample and check what was recorded.
//! Needs the .NET SDK and built sidecars:
//! `dotnet build sidecars/dotnet/Nodex.Weaver -c Release`, then `cargo test -p nodex-server --test run -- --ignored`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nodex_core::Project;
use nodex_ir::types::TypeShape;
use nodex_lang_api::LanguageFrontend;
use nodex_server::runner::RunState;
use nodex_server::{AppState, ServerEvent, runner};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the .NET SDK and a built Nodex.Weaver"]
async fn runs_the_sample_and_records_its_execution() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/csharp/Sample");
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_csharp::CSharpFrontend)];
    let project = Project::open(&root, frontends).unwrap();
    let state = AppState::new(project, "t".into(), 7420, Vec::new());
    let mut events = state.events.subscribe();

    runner::start(&state).expect("run starts");
    assert!(runner::start(&state).is_err(), "only one run at a time");

    let mut output = Vec::new();
    let mut final_state = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    while final_state.is_none() {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("run finishes in time").expect("event");
        match event {
            ServerEvent::RunOutput { lines } => output.extend(lines.into_iter().map(|l| l.text)),
            ServerEvent::RunStatus(s) if matches!(s.state, RunState::Exited | RunState::Failed) => final_state = Some(s),
            _ => {}
        }
    }
    let status = final_state.unwrap();
    assert_eq!(status.state, RunState::Exited, "{status:?}\n{}", output.join("\n"));
    assert_eq!(status.exit_code, Some(0));
    assert!(output.iter().any(|l| l == "Total: 195"), "{}", output.join("\n"));

    let snapshot = state.run.snapshot().trace.expect("trace recorded");
    let total = "Sample.Services.OrderService.TotalForCustomer(string)";
    assert_eq!(snapshot.calls.get(total), Some(&1));
    let counts = &snapshot.counts[total];
    // The loop body ran for each of the three orders.
    assert!(counts.values().any(|&c| c == 3), "{counts:?}");
    let values = &snapshot.vars[total];
    assert_eq!(values.get("total").map(String::as_str), Some("195"));

    let steps = state.run.recorder().unwrap().lock().unwrap().steps(total, 0, 1000);
    assert!(steps.total > 10);
    assert!(steps.steps.iter().any(|s| s.values.iter().any(|v| v == "total = 195")));
    assert!(steps.steps.iter().flat_map(|step| step.observations.iter().flatten()).any(|observation| {
        observation.name == "total" && observation.shape == TypeShape::scalar("Decimal")
    }), "{steps:?}");

    let mut items = vec![TypeShape::scalar("Int32"); 7];
    items.push(TypeShape::Tuple { items: vec![TypeShape::scalar("Int32"), TypeShape::scalar("String")] });
    items.push(TypeShape::scalar("Int32"));
    let tuple = TypeShape::Tuple { items };
    let main = state.run.recorder().unwrap().lock().unwrap().steps("Sample.Program.Main(string[])", 0, 1000);
    assert!(main.steps.iter().flat_map(|step| step.observations.iter().flatten()).any(|observation| {
        observation.name == "tuple" && observation.shape == tuple
    }), "{main:?}");

    // Async locals live in state-machine fields and are still captured.
    let ship = "Sample.Services.OrderService.ShipAsync(int)";
    let ship_vars = &snapshot.vars[ship];
    assert!(ship_vars.get("order").is_some_and(|v| v.contains("Id = 2")), "{ship_vars:?}");
    // ShipAsync(2) throws because order 2 is not paid; the throw statement is marked.
    let throws = &snapshot.throws[ship];
    assert_eq!(throws.values().sum::<u64>(), 1, "{throws:?}");
    let (node, _) = throws.iter().next().unwrap();
    assert!(snapshot.values[ship][node].contains("InvalidOperationException: Only paid orders can ship"));

    // The flame chart has the whole call tree.
    let calls = state.run.recorder().unwrap().lock().unwrap().calls(10_000);
    assert!(calls.spans.iter().any(|s| s.flow == total && s.depth > 0 && !s.open));
    assert!(calls.spans.iter().any(|s| s.flow == ship));
    assert!(calls.end > calls.start);
    // Every nested call knows its caller and the statement that called it.
    let nested = calls.spans.iter().find(|s| s.flow == total && s.depth > 0).expect("nested Total");
    let caller = calls.spans.iter().find(|s| Some(s.id) == nested.parent).expect("caller span");
    assert_eq!(caller.depth + 1, nested.depth);
    assert!(nested.call_site.is_some());
    assert!(!nested.args.is_empty() && nested.ret.is_some(), "{nested:?}");
    let tree = state.run.recorder().unwrap().lock().unwrap().span_tree(caller.id, 1000);
    assert!(tree.spans.iter().any(|s| s.id == nested.id));
}

/// Same scenario for the TypeScript sample: needs Node.js and
/// `pnpm --dir sidecars/node/nodex-ts install`, then `cargo test -p nodex-server --test run -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Node.js and an installed nodex-ts sidecar"]
async fn runs_the_typescript_sample_and_records_its_execution() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ts/Sample");
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_typescript::TypeScriptFrontend)];
    let project = Project::open(&root, frontends).unwrap();
    let state = AppState::new(project, "t".into(), 7440, Vec::new());
    let mut events = state.events.subscribe();

    runner::start(&state).expect("run starts");
    let mut output = Vec::new();
    let mut final_state = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    while final_state.is_none() {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("run finishes in time").expect("event");
        match event {
            ServerEvent::RunOutput { lines } => output.extend(lines.into_iter().map(|l| l.text)),
            ServerEvent::RunStatus(s) if matches!(s.state, RunState::Exited | RunState::Failed) => final_state = Some(s),
            _ => {}
        }
    }
    let status = final_state.unwrap();
    assert_eq!(status.state, RunState::Exited, "{status:?}\n{}", output.join("\n"));
    assert_eq!(status.exit_code, Some(0));
    assert!(output.iter().any(|l| l == "Total: 195"), "{}", output.join("\n"));

    let snapshot = state.run.snapshot().trace.expect("trace recorded");
    let total = "src/services/orderService.OrderService.totalForCustomer";
    assert_eq!(snapshot.calls.get(total), Some(&1), "{:?}", snapshot.calls);
    let counts = &snapshot.counts[total];
    assert!(counts.values().any(|&c| c == 3), "the loop body ran for each order: {counts:?}");
    assert_eq!(snapshot.vars[total].get("total").map(String::as_str), Some("195"));

    // Generators and async functions resume: they are called once, resumed several times.
    let describe = "src/services/orderService.OrderService.describeAll";
    assert_eq!(snapshot.calls.get(describe), Some(&1), "{:?}", snapshot.calls);
    let ship = "src/services/orderService.OrderService.ship";
    assert_eq!(snapshot.calls.get(ship), Some(&2));
    assert!(snapshot.vars[ship].get("order").is_some_and(|v| v.contains("Order {")), "{:?}", snapshot.vars[ship]);
    // ship(2) throws: the throw statement is marked.
    let throws = &snapshot.throws[ship];
    assert_eq!(throws.values().sum::<u64>(), 1, "{throws:?}");
    let (node, _) = throws.iter().next().unwrap();
    assert!(snapshot.values[ship][node].contains("Error: Only paid orders can ship"), "{:?}", snapshot.values[ship]);

    let calls = state.run.recorder().unwrap().lock().unwrap().calls(10_000);
    assert!(calls.spans.iter().any(|s| s.flow == total && s.depth > 0 && !s.open));
    assert!(calls.spans.iter().any(|s| s.flow == ship && s.resumed), "async continuation spans");

    // The `shipped` listener has its own Blueprint and runs once (order 1 ships).
    let listener = "src/main.main.on 'shipped'";
    assert_eq!(snapshot.calls.get(listener), Some(&1), "{:?}", snapshot.calls.keys().collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Node.js and the TypeScript sidecar"]
async fn run_captures_changing_javascript_types_in_recursive_steps() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ts/TraceTypes");
    let project = Project::open(&root, vec![Arc::new(nodex_lang_typescript::TypeScriptFrontend)]).unwrap();
    let id = project.flows().find(|flow| flow.name == "typed").unwrap().symbol.clone();
    let state = AppState::new(project, "t".into(), 7470, Vec::new());
    let mut events = state.events.subscribe();
    runner::start(&state).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("run finishes").expect("event");
        if let ServerEvent::RunStatus(status) = event
            && matches!(status.state, RunState::Exited | RunState::Failed)
        {
            assert_eq!(status.state, RunState::Exited, "{status:?}");
            break;
        }
    }
    let steps = state.run.recorder().unwrap().lock().unwrap().steps(&id, 0, 100);
    let captures = |name: &str| steps.steps.iter().flat_map(|step| step.observations.iter().flatten())
        .filter(|observation| observation.name == name).map(|observation| observation.shape.clone()).collect::<Vec<_>>();
    assert_eq!(captures("depth"), vec![TypeShape::scalar("Number"); 3], "{steps:?}");
    assert_eq!(captures("x"), [vec![TypeShape::scalar("String"); 3], vec![TypeShape::scalar("Number"); 3]].concat(), "{steps:?}");
    assert_eq!(captures("items"), vec![TypeShape::collection(nodex_ir::types::CollectionKind::Array, vec![TypeShape::Unknown]); 3]);
    assert!(steps.steps.iter().filter(|step| step.observations.iter().flatten().any(|observation| observation.name == "depth"))
        .all(|step| step.observations.iter().flatten().any(|observation| observation.name == "x" && observation.shape == TypeShape::scalar("String"))));
}

/// Same scenario for the Rust sample: needs cargo, then
/// `cargo test -p nodex-server --test run -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs cargo (builds an instrumented copy of the Rust sample)"]
async fn runs_the_rust_sample_and_records_its_execution() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rust/Sample");
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_rust::RustFrontend)];
    let project = Project::open(&root, frontends).unwrap();
    let state = AppState::new(project, "t".into(), 7460, Vec::new());
    let mut events = state.events.subscribe();

    runner::start(&state).expect("run starts");
    let mut output = Vec::new();
    let mut final_state = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    while final_state.is_none() {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("run finishes in time").expect("event");
        match event {
            ServerEvent::RunOutput { lines } => output.extend(lines.into_iter().map(|l| l.text)),
            ServerEvent::RunStatus(s) if matches!(s.state, RunState::Exited | RunState::Failed) => final_state = Some(s),
            _ => {}
        }
    }
    let status = final_state.unwrap();
    assert_eq!(status.state, RunState::Exited, "{status:?}\n{}", output.join("\n"));
    assert_eq!(status.exit_code, Some(0));
    assert!(output.iter().any(|l| l == "Total: 195"), "{}", output.join("\n"));

    let snapshot = state.run.snapshot().trace.expect("trace recorded");
    let total = "crate::services::order_service::OrderService::total_for_customer";
    assert_eq!(snapshot.calls.get(total), Some(&1), "{:?}", snapshot.calls);
    let counts = &snapshot.counts[total];
    assert!(counts.values().any(|&c| c == 3), "the loop body ran for each order: {counts:?}");
    assert_eq!(snapshot.vars[total].get("total").map(String::as_str), Some("195"), "{:?}", snapshot.vars[total]);
    let steps = state.run.recorder().unwrap().lock().unwrap().steps(total, 0, 1000);
    assert!(steps.steps.iter().flat_map(|step| step.observations.iter().flatten()).any(|observation| {
        observation.name == "total" && observation.shape == TypeShape::scalar("Int64")
    }), "{steps:?}");

    let ship = "crate::services::order_service::OrderService::ship";
    assert_eq!(snapshot.calls.get(ship), Some(&2));
    assert!(snapshot.vars[ship].get("shipped").is_some_and(|v| v.contains("Order {")), "{:?}", snapshot.vars[ship]);

    // The listener closure has its own Blueprint and runs once (order 1 ships); the worker
    // closure runs on another thread.
    let listener = "crate::main::on_shipped callback";
    assert_eq!(snapshot.calls.get(listener), Some(&1), "{:?}", snapshot.calls.keys().collect::<Vec<_>>());
    let worker = "crate::main::spawn callback";
    assert_eq!(snapshot.calls.get(worker), Some(&1));
    let calls = state.run.recorder().unwrap().lock().unwrap().calls(10_000);
    assert!(calls.spans.iter().any(|s| s.flow == total && s.depth > 0 && !s.open));
    let threads: std::collections::BTreeSet<i32> = calls.spans.iter().map(|s| s.thread).collect();
    assert!(threads.len() >= 2, "the worker ran on its own thread: {threads:?}");
}

/// A browser app (React + Vite): `npm install` in fixtures/web/react-vite first. Nodex starts
/// the dev server on an instrumented copy; the test plays the page by loading the app's
/// JavaScript in Node with the browser runtime's WebSocket, so no browser is needed.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Node.js, the nodex-ts sidecar and npm install in fixtures/web/react-vite"]
async fn runs_a_browser_app_under_its_dev_server() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/web/react-vite");
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(nodex_lang_typescript::TypeScriptFrontend)];
    let project = Project::open(&root, frontends).unwrap();
    // A real server so the page's WebSocket can reach `/trace`.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = AppState::new(project, "t".into(), port, Vec::new());
    tokio::spawn(axum::serve(listener, nodex_server::router(state.clone())).into_future());
    let mut events = state.events.subscribe();

    runner::start(&state).expect("run starts");
    struct StopOnPanic(Arc<AppState>);
    impl Drop for StopOnPanic {
        fn drop(&mut self) {
            self.0.run.stop();
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    let _guard = StopOnPanic(state.clone());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    let mut output = Vec::new();
    let url = loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("dev server starts in time").expect("event");
        match event {
            ServerEvent::RunOutput { lines } => output.extend(lines.into_iter().map(|l| l.text)),
            ServerEvent::RunStatus(s) if s.url.is_some() => break s.url.unwrap(),
            ServerEvent::RunStatus(s) if matches!(s.state, RunState::Exited | RunState::Failed) => panic!("{s:?}\n{}", output.join("\n")),
            _ => {}
        }
    };
    assert!(url.starts_with("http://localhost:5181"), "{url}");

    // The page as served by Vite: the instrumented module and the runtime with this run's URL.
    let cart = reqwest_text(&format!("{url}src/cart.ts")).await;
    assert!(cart.contains("__nx.step("), "{cart}");
    let runtime = reqwest_text(&format!("{url}__nodex_runtime.js")).await;
    let ws_url = runtime.split("const WS_URL = \"").nth(1).and_then(|r| r.split('"').next()).expect("runtime has the run's URL").to_owned();
    assert!(ws_url.contains(&format!(":{port}/trace?run=")), "{ws_url}");

    // Play the page: send one frame like the runtime does (Enter + Step of method 1).
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url.as_str()).await.expect("trace socket accepts the run token");
    let frame = nodex_trace::protocol::encode_frame(&[
        nodex_trace::Record { kind: nodex_trace::RecordKind::Sequence, thread: 42, micros: 0, method: 0, arg: 1, text: None },
        nodex_trace::Record { kind: nodex_trace::RecordKind::Enter, thread: 1, micros: 1, method: 1, arg: 0, text: None },
        nodex_trace::Record { kind: nodex_trace::RecordKind::Step, thread: 1, micros: 2, method: 1, arg: 1, text: None },
    ]);
    use futures_util::SinkExt;
    socket.send(tokio_tungstenite::tungstenite::Message::Binary(frame.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let snapshot = state.run.snapshot().trace.expect("trace recorded");
    assert!(snapshot.events >= 2, "{snapshot:?}");
    // A wrong token is refused.
    let bad = ws_url.replace("run=", "run=x");
    assert!(tokio_tungstenite::connect_async(bad.as_str()).await.is_err());

    assert!(state.run.stop());
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await.expect("stops in time").expect("event");
        if let ServerEvent::RunStatus(s) = event
            && s.state == RunState::Exited
        {
            assert_eq!(s.exit_code, Some(0), "a stop is not a failure");
            break;
        }
    }
}

async fn reqwest_text(url: &str) -> String {
    let url = url.to_owned();
    tokio::task::spawn_blocking(move || {
        let addr = url.trim_start_matches("http://");
        let (host, path) = addr.split_once('/').map(|(h, p)| (h.to_owned(), format!("/{p}"))).unwrap_or((addr.to_owned(), "/".into()));
        use std::io::{Read, Write};
        use std::net::ToSocketAddrs;
        // Node dev servers may listen on `localhost` as IPv6 only: try every address.
        let mut s = host
            .to_socket_addrs()
            .expect("resolves")
            .find_map(|a| std::net::TcpStream::connect_timeout(&a, Duration::from_secs(5)).ok())
            .expect("dev server reachable");
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).unwrap();
        body
    })
    .await
    .unwrap()
}
