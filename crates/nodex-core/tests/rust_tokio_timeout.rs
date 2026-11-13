use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/tokio-timeout/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn timeout(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { input, .. }
        if input.contains("when creating the timeout.")))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_timeout_separates_the_timer_from_the_wrapped_future_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/tokio-timeout");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("connection", "tokio::time::timeout(JOIN_TIMEOUT, socket.recv())", &["Ok(Some(Ok(message)))", "Ok(None)", "Ok(Some(Err(error)))", "Err(Elapsed)"][..]),
        ("durable", "tokio::time::timeout_at(deadline, receiver.recv())", &["Ok(Some(message))", "Ok(None)", "Err(Elapsed)"][..]),
        ("parenthesized", "by(deadline, async { Err(5) })", &["Ok(output)", "Err(Elapsed)"][..]),
        ("local_receive", "tokio::time::timeout(JOIN_TIMEOUT, socket.recv())", &["Ok(output)", "Err(Elapsed)"][..]),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| timeout(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, outputs), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = timeout(&graph).unwrap_or_else(|| panic!("Missing {name} timeout reading: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
        assert!(input.contains("ready result wins even at an expired deadline"));
        assert!(input.contains("the task can suspend"));
        assert!(input.contains("work started elsewhere may continue"));
        if matches!(name, "durable" | "parenthesized") {
            assert!(input.contains("Set the deadline to deadline"));
        } else {
            assert!(input.contains("Start the JOIN_TIMEOUT time limit"));
        }
        assert_eq!(branches.iter().map(|branch| branch.output.as_str()).collect::<Vec<_>>(), outputs);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["queued", "local_timeout"] {
        assert!(timeout(&flow(&mut project, name)).is_none());
    }
}
