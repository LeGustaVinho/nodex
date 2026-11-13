use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/nonzero-get/src/lib.rs");
const TARGET: &str = "core::num::nonzero::NonZero::get()";

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn detail(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Read the nonzero integer"))
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
fn scip_nonzero_get_reads_the_integer_without_a_lookup_or_failure_route() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/nonzero-get");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [("workers", "cores.get()"), ("signed", "SignedCount::get(count)"), ("explicit", "count.get()")];
    let before = cases.map(|(name, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| detail(graph).is_none()));
    assert!(expression(&before[2], |expr| expr.op.as_deref() == Some("list.get")).is_some());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = detail(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("underlying integer type") && text.contains("guaranteed to be different from zero")));
        let get = expression(&graph, |expr| expr.target.as_deref() == Some(TARGET)).unwrap();
        assert_eq!(get.concept, Concept::Call);
        assert!(get.op.is_none());
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }

    let lookup = flow(&mut project, "lookup");
    assert!(detail(&lookup).is_none());
    assert!(expression(&lookup, |expr| expr.op.as_deref() == Some("list.get")).is_some());
    assert!(lookup.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Paths { branches, .. } if branches[1].output == "None")));
    assert!(detail(&flow(&mut project, "local_zero")).is_none());
}
