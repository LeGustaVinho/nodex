use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/tokio-join/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn joined(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if matches!(label.as_str(), "Wait for every future" | "Wait for every future to succeed")))
}

fn async_all(expr: &Expr) -> bool {
    expr.op.as_deref() == Some("async.all") || expr.children.iter().any(async_all)
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_tokio_join_results_and_keeps_propagation_separate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/tokio-join");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("prepare_batch", "tokio::try_join", true, false),
        ("write_batch", "tokio::join", false, false),
        ("ordered", "gather", true, true),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| joined(graph).is_none()));
    assert!(before.iter().all(|graph| !graph.nodes.iter().filter_map(|node| node.expr.as_ref()).any(async_all)));
    assert!(flow(&mut project, "join_all_control").nodes.iter().filter_map(|node| node.expr.as_ref()).any(async_all));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, macro_path, fallible, biased), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = joined(&graph).unwrap_or_else(|| panic!("Missing {name} join reading: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], macro_path);
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("concurrently within the current task"));
        assert!(text.contains("a tuple") && text.contains("the order the futures are written"));
        assert_eq!(text.contains("With `biased;`"), biased);
        if fallible {
            assert!(text.contains("return Ok with a tuple of their values"));
            assert!(text.contains("drop the futures still in progress"));
            assert!(text.contains("effects already performed may still be observable"));
        } else {
            assert!(text.contains("keep that error in its tuple position and keep waiting for the others"));
        }
        assert!(!graph.nodes.iter().filter_map(|node| node.expr.as_ref()).any(async_all));
        if name == "prepare_batch" {
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
                if &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize] == "?" && branches[0].label == "Success")));
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    assert!(joined(&flow(&mut project, "local")).is_none());
}
