use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/sort-reading/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn sort(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Steps { label, .. }
        if label == "Sort the original slice with a comparison function"))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_mutation_comparison_and_stability_at_slice_sorts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/sort-reading");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [("nearest", "sort_by", "by_distance"), ("micro_jumps", "sort_by", "|a, b|"), ("unstable", "sort_unstable_by", "compare")];
    let before = cases.map(|(name, _, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| sort(graph).is_none()));
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for ((name, method, compare), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = sort(&graph).unwrap_or_else(|| panic!("{name}: {:?}", graph.reading));
        let source = |span: &nodex_ir::Span| &SOURCE[span.start_byte as usize..span.end_byte as usize];
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(source(&site.span), method);
        let ReadingContent::Steps { steps, .. } = &site.content else { unreachable!() };
        assert!(source(&steps[0].span).starts_with(compare));
        assert!(steps[0].action.contains("shared references"));
        assert_eq!(source(&steps[1].span), "contacts");
        assert_eq!(steps[1].action.contains("keep their original relative order"), method == "sort_by");
        assert_eq!(steps[1].action.contains("may change their relative order"), method == "sort_unstable_by");
        assert!(steps[2].action.starts_with("Return ()."));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    assert!(sort(&flow(&mut project, "homonym")).is_none());
    let keys = flow(&mut project, "entries");
    assert!(keys.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Steps { label, steps }
        if label == "Sort the original slice by keys" && steps[0].action.contains("more than once")
            && steps[1].action.contains("equal keys keep their original relative order"))));
    let callback = flow(&mut project, "sort_by callback");
    let entry = callback.nodes.iter().find(|node| node.concept == Concept::Entry).unwrap();
    assert!(callback.reading.iter().flatten().any(|site| site.node == entry.id
        && &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize] == "|a, b|"
        && matches!(&site.content, ReadingContent::Detail { label, text, .. }
            if label == "Compare two elements during sorting" && text.contains("shared references"))));
}
