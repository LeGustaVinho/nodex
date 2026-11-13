use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/mem-update/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/mem-update");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, function: &str) -> FlowGraph {
    let symbol = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == function)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn updates(graph: &FlowGraph) -> Vec<&ReadingSite> {
    let mut cards = graph
        .reading
        .iter()
        .flatten()
        .filter(|site| {
            matches!(&site.content, ReadingContent::Steps { label, .. }
                if label == "Take the previous value and install the default"
                    || label == "Replace and return the previous value")
        })
        .collect::<Vec<_>>();
    cards.sort_by_key(|site| site.span.start_byte);
    cards
}

fn source(span: &Span) -> &'static str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

#[test]
fn rust_mem_update_spelling_alone_does_not_establish_the_update() {
    let mut project = project();
    for function in [
        "reconcile_locks",
        "resolve_hit",
        "forwarded",
        "local_updates",
        "option_take",
    ] {
        assert!(updates(&flow(&mut project, function)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_the_new_destination_and_the_returned_previous_value() {
    let mut project = project();
    let before = flow(&mut project, "resolve_hit");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, expected) in [
        (
            "reconcile_locks",
            &[
                (
                    "take",
                    "&mut state.locks",
                    "Default::default()",
                    "state.locks",
                ),
                (
                    "take",
                    "&mut state.pending_locks",
                    "Default::default()",
                    "state.pending_locks",
                ),
            ][..],
        ),
        (
            "resolve_hit",
            &[(
                "replace",
                "&mut encounter.hit_layers",
                "outer",
                "encounter.hit_layers",
            )][..],
        ),
        (
            "forwarded",
            &[
                (
                    "install",
                    "slot",
                    "new_name",
                    "the place referred to by slot",
                ),
                (
                    "reset",
                    "slot",
                    "Default::default()",
                    "the place referred to by slot",
                ),
            ][..],
        ),
    ] {
        let graph = flow(&mut project, function);
        let cards = updates(&graph);
        assert_eq!(
            cards.len(),
            expected.len(),
            "{function}: {:?}",
            graph.reading
        );
        for (site, (callee, destination, replacement, place)) in cards.into_iter().zip(expected) {
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert!(site.path.is_some(), "{site:?}");
            assert_eq!(source(&site.span), *callee);
            let ReadingContent::Steps { steps, .. } = &site.content else {
                panic!("{site:?}")
            };
            assert_eq!(steps.len(), 2);
            assert_eq!(steps[0].timing, "Destination if the call returns");
            assert_eq!(source(&steps[0].span), *destination);
            assert!(
                steps[0].action.contains(replacement)
                    && steps[0].action.ends_with(&format!("into {place}."))
            );
            assert_eq!(steps[1].timing, "Returned value");
            assert_eq!(
                steps[1].action,
                format!("Return the previous value from {place} without dropping it in this call.")
            );
        }
    }

    let after = flow(&mut project, "resolve_hit");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    let with_fallback = flow(&mut project, "resolve_hit_with_fallback");
    let fallback = with_fallback
        .reading
        .iter()
        .flatten()
        .find(|site| {
            matches!(&site.content, ReadingContent::Paths { .. })
                && source(&site.span).contains("unwrap_or_else")
        })
        .expect("unwrap_or_else retains its own reading");
    assert_ne!(fallback.path, updates(&with_fallback)[0].path);
    assert_eq!(fallback.node, updates(&with_fallback)[0].node);

    for function in ["local_updates", "option_take"] {
        assert!(updates(&flow(&mut project, function)).is_empty());
    }
}
