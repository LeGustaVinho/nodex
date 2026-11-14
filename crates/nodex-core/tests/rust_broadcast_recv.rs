use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/broadcast-recv/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn receives(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Paths { input, .. }
        if input.contains("for this broadcast receiver through"))).collect()
}

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_reads_broadcast_outcomes_at_each_select_wait_and_direct_await() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/broadcast-recv");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("gateway", &["results.recv()", "publications.recv()"][..]),
        ("awaited", &["broadcast::Receiver::recv(rx)"][..]),
    ];
    let before = cases.map(|(name, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| receives(graph).is_empty()));
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, waits), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let mut sites = receives(&graph);
        sites.sort_by_key(|site| site.span.start_byte);
        assert_eq!(sites.len(), waits.len(), "{name}: {:?}", graph.reading);
        for (site, wait) in sites.into_iter().zip(waits) {
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], *wait);
            let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
            assert_eq!(input.contains("still pending when another branch wins"), name == "gateway");
            assert_eq!(input.starts_with("When this branch is enabled and polled"), name == "gateway");
            assert_eq!(branches.iter().map(|branch| branch.output.as_str()).collect::<Vec<_>>(),
                ["Ok(message)", "Err(RecvError::Lagged(n))", "Err(RecvError::Closed)"]);
            assert!(branches[1].action.contains("overwrote n messages"));
            assert!(branches[1].action.contains("oldest message still retained"));
            assert!(branches[2].action.contains("no buffered messages left to read"));
            assert!(branches.iter().all(|branch| branch.pin.is_none()));
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["pending", "local_macro"] {
        assert!(receives(&flow(&mut project, name)).is_empty());
    }
}
