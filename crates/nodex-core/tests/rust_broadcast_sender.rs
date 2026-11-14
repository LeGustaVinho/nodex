use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/broadcast-sender/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn broadcast_sender(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| match &site.content {
        ReadingContent::Detail { label, .. } => label == "Subscribe to future broadcasts",
        ReadingContent::Paths { branches, .. } => branches.first().is_some_and(|branch| branch.label == "At least one receiver is active"),
        _ => false,
    })
}

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_distinguishes_broadcast_subscription_and_send_from_other_channels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/broadcast-sender");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [("subscribe", "subscribe"), ("publish", "send")];
    let before = cases.map(|(name, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| broadcast_sender(graph).is_none()));
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for ((name, method), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = broadcast_sender(&graph).unwrap_or_else(|| panic!("{name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], method);
        assert!(site.path.is_some());
        match &site.content {
            ReadingContent::Detail { text, .. } => {
                assert_eq!(method, "subscribe");
                assert!(text.contains("next send") && text.contains("outside the subscription"));
            }
            ReadingContent::Paths { input, branches, .. } => {
                assert_eq!(method, "send");
                assert!(input.contains("without waiting for receivers"));
                assert_eq!(branches[0].output, "Ok(receiver count)");
                assert_eq!(branches[1].output, "Err(SendError(value))");
                assert!(branches.iter().all(|branch| branch.pin.is_none()));
            }
            _ => unreachable!(),
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    assert!(broadcast_sender(&flow(&mut project, "custom")).is_none());
}
