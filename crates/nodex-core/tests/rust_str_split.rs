use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/str-split/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn split_card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| match &site.content {
        ReadingContent::Detail { label, .. } => label == "Iterate over pieces of text",
        ReadingContent::Paths { branches, .. } => branches.first().is_some_and(|branch| branch.output == "Some((before, after))"),
        _ => false,
    })
}

fn has_text_split(expr: &Expr) -> bool {
    expr.op.as_deref() == Some("text.split") || expr.children.iter().any(has_text_split)
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_str_split_distinguishes_an_iterator_from_a_pair_of_borrowed_slices() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/str-split");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("ticket", "token.split('.')", false),
        ("decimal", "text.split_once('.')", true),
        ("owner", "value.split_once('=')", true),
        ("collect_pieces", "text.split(',')", false),
        ("qualified", "str::split_once(text, |ch| ch == '-' || ch == '_')", true),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| split_card(graph).is_none()));
    assert!(before.iter().flat_map(|graph| &graph.nodes).filter_map(|node| node.expr.as_ref()).all(|expr| !has_text_split(expr)));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling, once), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = split_card(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        match &site.content {
            ReadingContent::Detail { text, .. } if !once => {
                assert!(text.contains("As the iterator is consumed"));
                assert!(text.contains("borrowed &str"));
                assert!(text.contains("empty pieces"));
                assert!(text.contains("entire text is the only piece"));
            }
            ReadingContent::Paths { branches, .. } if once => {
                assert_eq!(branches.len(), 2);
                assert_eq!(branches[0].output, "Some((before, after))");
                assert!(branches[0].action.contains("after the first match as two &str slices"));
                assert_eq!(branches[1].output, "None");
                assert!(branches.iter().all(|branch| branch.pin.is_none()));
            }
            content => panic!("Wrong reading in {name}: {content:?}"),
        }
        assert!(graph.nodes.iter().filter_map(|node| node.expr.as_ref()).all(|expr| !has_text_split(expr)));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
        if name == "collect_pieces" {
            assert!(graph.reading.iter().flatten().any(|entry| matches!(&entry.content,
                ReadingContent::Steps { label, .. } if label == "Follow an item through the iterator")));
        }
    }
    for name in ["split_slice", "local_split"] {
        let graph = flow(&mut project, name);
        assert!(split_card(&graph).is_none());
        assert!(graph.nodes.iter().filter_map(|node| node.expr.as_ref()).all(|expr| !has_text_split(expr)));
    }
}
