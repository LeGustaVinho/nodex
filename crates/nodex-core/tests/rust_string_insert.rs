use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/string-insert/src/lib.rs");
const TARGET: &str = "alloc::string::String::insert()";

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn detail(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label.starts_with("Insert a character")))
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
fn scip_string_insert_explains_the_utf8_byte_offset_and_mutation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/string-insert");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("js_to_fixed", "text.insert(0, '-')", true),
        ("insert_at", "text.insert(index, character)", false),
        ("qualified", "String::insert(text, 0, '+')", true),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| detail(graph).is_none()));
    assert!(expression(&before[0], |expr| expr.op.as_deref() == Some("list.add")).is_some());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, spelling, at_start), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = detail(&graph).unwrap_or_else(|| panic!("Missing {name} card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], spelling);
        assert!(site.path.is_some());
        let ReadingContent::Detail { label, text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("changing the existing String"));
        if at_start {
            assert_eq!(label, "Insert a character at the start");
            assert!(text.contains("Byte offset 0 is the start"));
            assert!(!text.contains("panics"));
        } else {
            assert_eq!(label, "Insert a character at a byte offset");
            assert!(text.contains("equal to the byte length appends"));
            assert!(text.contains("offset is greater than the byte length"));
            assert!(text.contains("falls inside a UTF-8 character"));
        }
        let insert = expression(&graph, |expr| expr.target.as_deref() == Some(TARGET)).unwrap();
        assert!(insert.op.is_none());
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }

    for name in ["insert_item", "insert_key"] {
        let graph = flow(&mut project, name);
        assert!(detail(&graph).is_none());
        assert!(expression(&graph, |expr| expr.op.as_deref() == Some("list.add")).is_some());
    }
    assert!(detail(&flow(&mut project, "local_insert")).is_none());
}
