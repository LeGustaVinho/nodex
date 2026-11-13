use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/iterator-zip/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn zip_card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Pair items from two inputs"))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_standard_zip_without_changing_the_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/iterator-zip");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("compare_ids", "x.iter().zip(y)", "x.iter()", "y"),
        ("same_baselines", "current.iter().zip(known)", "current.iter()", "known"),
        ("collect_pairs", "left.iter().zip(right)", "left.iter()", "right"),
        ("qualified", "Iterator::zip(left, right)", "left", "right"),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| zip_card(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, left, right), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = zip_card(&graph).unwrap_or_else(|| panic!("Missing {name} zip reading: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains(&format!("{left} first and the item from {right} second")));
        assert!(text.contains("current positions of both inputs"));
        assert!(text.contains("When advancing from the front, try the first input first"));
        assert!(text.contains("yields None for that request"));
        assert!(text.contains("may already have supplied an item that goes unpaired"));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
        if name == "compare_ids" {
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { input, .. }
                if input.contains("Count from zero at the next item this iterator would yield"))));
        } else if name == "collect_pairs" {
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Steps { steps, .. }
                if steps.iter().any(|step| step.action.contains("if both inputs supply an item")))));
        }
    }
    let option = flow(&mut project, "option_zip");
    assert!(zip_card(&option).is_none());
    assert!(option.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].output == "Some((first, second))")));
    for name in ["parallel_zip", "custom_zip"] {
        assert!(zip_card(&flow(&mut project, name)).is_none());
    }
}
