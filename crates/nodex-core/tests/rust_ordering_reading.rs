use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/ordering-reading/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn ordering(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph.reading.iter().flatten().filter(|site| match &site.content {
        ReadingContent::Detail { label, .. } => label == "Compare floating-point values in a total order",
        ReadingContent::Paths { branches, .. } => branches.first().is_some_and(|branch| branch.label == "First comparison is Equal"),
        _ => false,
    }).collect()
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_eager_and_deferred_ordering_tie_breakers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/ordering-reading");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [("by_distance", "then"), ("by_id", "then_with"), ("qualified", "then_with")];
    let before = cases.map(|(name, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| ordering(graph).is_empty()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, method), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let sites = ordering(&graph);
        assert_eq!(sites.len(), 2, "{name}: {:?}", graph.reading);
        for site in sites {
            let written = &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize];
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert!(site.path.is_some());
            match &site.content {
                ReadingContent::Detail { text, .. } => {
                    assert_eq!(written, "total_cmp");
                    assert!(text.contains("NaN") && text.contains("negative zero"));
                }
                ReadingContent::Paths { input, branches, .. } => {
                    assert_eq!(written, method);
                    assert_eq!(branches[0].action.contains("Call the tie-breaking function"), method == "then_with");
                    assert_eq!(branches[1].action.contains("skip calling"), method == "then_with");
                    assert_eq!(input.contains("even when the first comparison already decides"), method == "then");
                    assert!(branches.iter().all(|branch| branch.pin.is_none()));
                }
                _ => unreachable!(),
            }
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    assert!(ordering(&flow(&mut project, "homonyms")).is_empty());
    let callback = flow(&mut project, "then_with callback");
    let entry = callback.nodes.iter().find(|node| node.concept == Concept::Entry).unwrap();
    assert!(callback.reading.iter().flatten().any(|site| site.node == entry.id
        && &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize] == "||"
        && matches!(&site.content, ReadingContent::Detail { label, text, .. }
            if label == "Break a tie between equal values" && text.contains("only when the first comparison returns Equal"))));
}
