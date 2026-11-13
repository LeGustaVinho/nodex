use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}, types::TypeShape};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/rust-length/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn detail(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if matches!(label.as_str(), "Count UTF-8 bytes" | "Count elements")))
}

fn length(graph: &FlowGraph) -> &Expr {
    graph.nodes.iter().filter_map(|node| node.expr.as_ref()).find(|expr| expr.label == "len").unwrap()
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_length_distinguishes_utf8_bytes_from_elements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/rust-length");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("encoded_length", "json.len()", "alloc::string::String::len()", "Count UTF-8 bytes"),
        ("fractional_length", "fraction.len()", "core::str::str::len()", "Count UTF-8 bytes"),
        ("json_items", "items.len()", "alloc::vec::Vec::len()", "Count elements"),
        ("slice_items", "items.len()", "core::slice::[T::]len().", "Count elements"),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| detail(graph).is_none() && length(graph).op.as_deref() == Some("text.length")));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling, target, expected), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = detail(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        let ReadingContent::Detail { label, text, .. } = &site.content else { unreachable!() };
        assert_eq!(label, expected);
        if expected == "Count UTF-8 bytes" {
            assert!(text.contains("the character 'é' occupies two bytes"));
        } else {
            assert!(text.contains("Each element counts once"));
        }
        assert!(text.contains("as a usize"));
        assert_eq!(length(&graph).target.as_deref(), Some(target));
        assert_ne!(length(&graph).op.as_deref(), Some("text.length"));
        assert_eq!(length(&graph).type_info.as_ref().unwrap().shape, TypeShape::scalar("UIntSize"));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["object_entries", "local_length"] {
        let graph = flow(&mut project, name);
        assert!(detail(&graph).is_none());
        assert_ne!(length(&graph).op.as_deref(), Some("text.length"));
    }
}
