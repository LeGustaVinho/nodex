use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/vec-drain/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn detail(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Remove elements with Vec::drain"))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_vec_drain_explains_removed_elements_and_when_they_are_dropped() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/vec-drain");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("write_batch", "batch.drain(..)", true, false),
        ("trim_visual", "retained.drain(..excess)", false, true),
        ("early_return", "batch.drain(..)", true, false),
        ("early_break", "batch.drain(..)", true, false),
        ("saved", "Vec::drain(batch, ..)", true, false),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| detail(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling, whole, discarded), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = detail(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("holds a mutable borrow"));
        assert!(text.contains("yields removed elements by value"));
        assert!(text.contains("drops any removed elements that it has not yielded"));
        assert_eq!(text.contains("Remove every element"), whole);
        assert_eq!(text.contains("causes a panic"), !whole);
        assert_eq!(text.contains("This statement drops the iterator without using it"), discarded);
        if name == "trim_visual" {
            assert!(text.contains("from the start of retained up to, but not including, index excess"), "{text}");
        }
        if let Some(concept) = match name { "early_return" => Some(Concept::Return), "early_break" => Some(Concept::Break), _ => None } {
            assert!(graph.nodes.iter().any(|node| node.concept == concept));
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["deque", "local_drain"] {
        assert!(detail(&flow(&mut project, name)).is_none());
    }
}
