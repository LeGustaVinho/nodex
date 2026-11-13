use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    Concept, FlowEdgeKind, FlowGraph, Language, SymbolId, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/map-callback-entry/src/lib.rs");
const ITER: &str = "Transform an item from the iterator";
const OPTION: &str = "Transform the value inside Some";
const OPTION_ASYNC: &str = "Create a future for the value inside Some";
const ITER_ASYNC: &str = "Create a future for an iterator item";

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/map-callback-entry");
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

fn children(project: &mut Project, owner: &SymbolId) -> Vec<FlowGraph> {
    let ids: Vec<_> = project
        .structure()
        .symbols
        .iter()
        .filter(|symbol| symbol.parent.as_ref() == Some(owner) && symbol.has_flow)
        .map(|symbol| symbol.id.clone())
        .collect();
    ids.iter().map(|id| project.flow(id).unwrap()).collect()
}

fn roles(graph: &FlowGraph) -> Vec<(&ReadingSite, &str, &str)> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Detail { label, text, .. } = &site.content else {
                return None;
            };
            matches!(label.as_str(), ITER | OPTION | OPTION_ASYNC | ITER_ASYNC).then_some((
                site,
                label.as_str(),
                text.as_str(),
            ))
        })
        .collect()
}

fn assert_entry(graph: &FlowGraph, expected: &str, highlighted: &str) {
    let entries = roles(graph);
    let [(site, label, text)] = entries.as_slice() else {
        panic!("{}: {:?}", graph.name, graph.reading)
    };
    assert_eq!(*label, expected);
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    assert_eq!(site.path, None);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.id == site.node && node.concept == Concept::Entry)
    );
    assert_eq!(site.span.file, "src/lib.rs");
    assert_eq!(
        &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize],
        highlighted
    );
    match expected {
        ITER => assert!(
            text.contains("saves this function for later")
                && text.contains("When a consumer reads a mapped item")
        ),
        OPTION => assert!(
            text.contains("Some passes its value to this function once")
                && text.contains("None skips this function")
        ),
        OPTION_ASYNC => assert!(
            text.contains("wraps the future")
                && text.contains("body runs when the future is polled")
        ),
        ITER_ASYNC => assert!(
            text.contains("yields the future")
                && text.contains("body runs when the future is polled")
        ),
        _ => unreachable!(),
    }
}

#[test]
fn map_spelling_alone_keeps_callback_entry_generic() {
    let mut project = project();
    for name in [
        "json_lengths",
        "iterator_ufcs",
        "option_method",
        "option_ufcs",
    ] {
        let parent = flow(&mut project, name);
        let children = children(&mut project, &parent.symbol);
        assert_eq!(children.len(), 1);
        assert!(roles(&children[0]).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_the_standard_map_callback_at_its_own_entry() {
    let mut project = project();
    let baseline = flow(&mut project, "json_lengths");
    let baseline_children = children(&mut project, &baseline.symbol);
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, expected, highlighted) in [
        ("json_lengths", ITER, "|(key, item)|"),
        ("iterator_ufcs", ITER, "|word|"),
        ("option_method", OPTION, "|item|"),
        ("option_ufcs", OPTION, "|item|"),
        ("option_async", OPTION_ASYNC, "|item|"),
        ("iterator_async", ITER_ASYNC, "|item|"),
    ] {
        let parent = flow(&mut project, name);
        assert!(roles(&parent).is_empty());
        assert!(
            parent
                .reading
                .iter()
                .flatten()
                .any(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Create a function"))
        );
        let callbacks = children(&mut project, &parent.symbol);
        assert_eq!(callbacks.len(), 1);
        assert_entry(&callbacks[0], expected, highlighted);
    }
    let after = flow(&mut project, "json_lengths");
    let callback = children(&mut project, &after.symbol);
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&baseline), execution(&after));
    assert_eq!(execution(&baseline_children[0]), execution(&callback[0]));

    for name in ["local_map", "result_map", "named_mapper", "inline_mapper"] {
        let parent = flow(&mut project, name);
        assert!(roles(&parent).is_empty());
        assert!(
            children(&mut project, &parent.symbol)
                .iter()
                .all(|graph| roles(graph).is_empty())
        );
    }
    let parent = flow(&mut project, "nested");
    assert!(roles(&parent).is_empty());
    let outer = children(&mut project, &parent.symbol);
    assert_entry(&outer[0], ITER, "|item|");
    let nested = children(&mut project, &outer[0].symbol);
    let adjust = nested
        .iter()
        .find(|graph| graph.name.starts_with("adjust"))
        .unwrap();
    assert!(roles(adjust).is_empty());
    let mapper = nested
        .iter()
        .find(|graph| graph.name.starts_with("map"))
        .unwrap();
    assert_entry(mapper, OPTION, "|value|");
}
