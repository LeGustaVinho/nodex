//! End-to-end live trace: build, weave, run the C# sample and check what was recorded.
//! Needs the .NET SDK and built sidecars:
//! `dotnet build sidecars/dotnet/Nodex.Weaver -c Release`, then `cargo test -p nodex-server --test run -- --ignored`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nodex_core::Project;
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
}
