use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/inclusive-contains/src/lib.rs");
const TARGET: &str = "core::ops::range::RangeInclusive::contains()";

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn detail(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Check both inclusive bounds"))
}

fn expression<'a>(graph: &'a FlowGraph, predicate: impl Fn(&Expr) -> bool) -> Option<&'a Expr> {
    fn find<'a>(expr: &'a Expr, predicate: &impl Fn(&Expr) -> bool) -> Option<&'a Expr> {
        if predicate(expr) { return Some(expr); }
        expr.children.iter().find_map(|child| find(child, predicate))
    }
    graph.nodes.iter().filter_map(|node| node.expr.as_ref()).find_map(|expr| find(expr, &predicate))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_inclusive_range_membership_keeps_bounds_and_short_circuit_reading() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/inclusive-contains");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("combat", "(2..=64).contains(&ids.len())", "ids.len() is at least 2 and at most 64"),
        ("comparable", "RangeInclusive::contains(range, item)", "item is at or above its current start"),
        ("exhausted", "range.contains(&item)", "the range has not been exhausted"),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| detail(graph).is_none()));
    assert!(expression(&before[0], |expr| expr.op.as_deref() == Some("text.contains")).is_some());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling, expected), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = detail(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains(expected) && text.contains("Both endpoints are included")
                && text.contains("including incomparable values, return false")));
        let contains = expression(&graph, |expr| expr.target.as_deref() == Some(TARGET)).unwrap();
        assert_ne!(contains.op.as_deref(), Some("text.contains"));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
        if name == "combat" {
            assert!(graph.reading.iter().flatten().any(|entry| entry.node == site.node && matches!(&entry.content,
                ReadingContent::Detail { label, .. } if label == "Range includes the end")));
            assert!(graph.reading.iter().flatten().any(|entry| entry.node == site.node && matches!(&entry.content,
                ReadingContent::Paths { branches, .. } if branches.iter().any(|branch| branch.action == "Skip the right side"))));
        }
    }

    for name in ["exclusive", "text_contains", "slice_contains"] {
        let graph = flow(&mut project, name);
        assert!(detail(&graph).is_none());
        assert!(expression(&graph, |expr| expr.op.as_deref() == Some("text.contains")).is_some());
    }
    assert!(detail(&flow(&mut project, "local_contains")).is_none());
}
