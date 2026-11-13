use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/disjoint-mutable/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn borrow_parts(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Paths { branches, .. }
        if branches.first().is_some_and(|branch| branch.label == "All parts are valid and disjoint")))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_multiple_mutable_borrows_without_changing_execution_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/disjoint-mutable");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("collide", "ships\n        .get_disjoint_mut([order[i], order[j]])", "[order[i], order[j]]", "ships"),
        ("intervals", "<[i32]>::get_disjoint_mut(slots, [3..4, 0..2])", "[3..4, 0..2]", "slots"),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| borrow_parts(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, indices, slice), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = borrow_parts(&graph).unwrap_or_else(|| panic!("Missing reading for {name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(SOURCE[site.span.start_byte as usize..site.span.end_byte as usize].replace("\r\n", "\n"), call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
        assert!(input.contains(indices) && input.contains(slice));
        assert!(branches[0].action.contains("in the order supplied"));
        assert!(branches[0].action.contains("updates the original"));
        assert_eq!(branches[1].output, "Err(IndexOutOfBounds)");
        assert_eq!(branches[2].output, "Err(OverlappingIndices)");
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    assert!(borrow_parts(&flow(&mut project, "homonym")).is_none());
}
