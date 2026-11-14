use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite, ReadingStep},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/vec-extend/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/vec-extend");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap()
        .id
        .clone();
    project.flow(&id).unwrap()
}

fn source(span: &Span) -> &str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn extends(graph: &FlowGraph) -> Vec<(&ReadingSite, &[ReadingStep])> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Steps { label, steps } = &site.content else {
                return None;
            };
            (label == "Append items to the vector").then_some((site, steps.as_slice()))
        })
        .collect()
}

#[test]
fn spelling_alone_does_not_establish_vector_extension_behavior() {
    let mut project = project();
    for name in ["ammunition_ids", "settle", "borrowed_items", "homonyms"] {
        assert!(extends(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_which_values_extend_the_existing_vector() {
    let mut project = project();
    let before = flow(&mut project, "settle");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, count, option, copy) in [
        ("ammunition_ids", 2, true, false),
        ("settle", 1, false, false),
        ("borrowed_items", 1, false, true),
        ("borrowed_option", 1, true, true),
        ("reference_items", 1, false, false),
        ("local_binding", 1, true, false),
        ("constructor", 1, true, false),
        ("emit", 1, true, false),
        ("multiple", 2, false, false),
    ] {
        let graph = flow(&mut project, name);
        let cards = extends(&graph);
        assert_eq!(cards.len(), count, "{name}: {:?}", graph.reading);
        for (site, steps) in cards {
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert_eq!(source(&site.span), "extend");
            assert_eq!(
                steps[0].action.contains("zero or one item"),
                option,
                "{name}: {:?}",
                steps[0]
            );
            assert_eq!(
                steps[1].action.contains("Copy each referenced value"),
                copy,
                "{name}: {:?}",
                steps[1]
            );
            assert!(steps[1].action.contains("to the end of"));
            assert!(steps[1].action.contains("in the source's order"));
            assert!(steps[2].action.contains("returns () (unit)"));
            if name == "settle" {
                assert!(source(&steps[0].span).starts_with("ships.iter().map"));
                assert!(steps[0].action.contains("callback runs during this call"));
            }
        }
    }
    assert!(extends(&flow(&mut project, "homonyms")).is_empty());
    let after = flow(&mut project, "settle");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
}
