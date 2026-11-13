use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingStep},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/future-preparation/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/future-preparation");
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

fn preparations(graph: &FlowGraph) -> Vec<(&str, &str, &[ReadingStep])> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Steps { label, steps } = &site.content else {
                return None;
            };
            if !matches!(
                label.as_str(),
                "Make a future from a value"
                    | "Put the future in a pinned box"
                    | "Put the value in a pinned box"
            ) {
                return None;
            }
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert_eq!(site.span.file, "src/lib.rs");
            let token = &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize];
            Some((label.as_str(), token, steps.as_slice()))
        })
        .collect()
}

#[test]
fn syntax_alone_does_not_classify_ready_or_pin() {
    let mut project = project();
    for name in ["now", "after_durable", "boxed_value", "homonyms"] {
        assert!(preparations(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_when_values_and_pinned_futures_progress() {
    let mut project = project();
    let before = flow(&mut project, "after_durable");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    let now = flow(&mut project, "now");
    let cards = preparations(&now);
    assert_eq!(cards.len(), 2);
    let (label, token, steps) = cards.iter().find(|card| card.1 == "resolved").unwrap();
    assert_eq!((*label, *token), ("Make a future from a value", "resolved"));
    assert!(steps[0].action.contains("including any effects"));
    assert_eq!(steps[1].timing, "On the first poll");
    assert!(steps[1].action.contains("Poll::Ready(value)"));
    assert!(
        cards
            .iter()
            .any(|card| card.0 == "Put the future in a pinned box" && card.1 == "pin")
    );

    let after_durable = flow(&mut project, "after_durable");
    let cards = preparations(&after_durable);
    let [(label, token, steps)] = cards.as_slice() else {
        panic!("{:?}", after_durable.reading)
    };
    assert_eq!((*label, *token), ("Put the future in a pinned box", "pin"));
    assert!(
        steps[1]
            .action
            .contains("Polling the future is a separate operation")
    );
    assert!(steps[1].action.contains("box handle can move"));

    let boxed = flow(&mut project, "boxed_value");
    let cards = preparations(&boxed);
    let [(label, token, steps)] = cards.as_slice() else {
        panic!("{:?}", boxed.reading)
    };
    assert_eq!((*label, *token), ("Put the value in a pinned box", "pin"));
    assert!(steps[1].action.contains("Pin<Box<T>>"));
    assert!(steps[1].action.contains("box handle can move"));

    assert!(preparations(&flow(&mut project, "homonyms")).is_empty());
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after_durable));
}
