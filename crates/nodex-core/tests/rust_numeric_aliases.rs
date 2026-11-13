use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowGraph, Language, ValueBasis, reading::ReadingContent};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/numeric-aliases/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/numeric-aliases");
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

fn casts(flow: &FlowGraph) -> Vec<(&str, &str)> {
    let mut cards = flow
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Detail { label, text, .. } = &site.content else {
                return None;
            };
            if !label.starts_with("Convert ") {
                return None;
            }
            let span = &site.span;
            assert_eq!(span.file, "src/lib.rs");
            assert_eq!(
                &SOURCE[span.start_byte as usize..span.end_byte as usize],
                "as"
            );
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            Some((span.start_byte, label.as_str(), text.as_str()))
        })
        .collect::<Vec<_>>();
    cards.sort_by_key(|card| card.0);
    cards
        .into_iter()
        .map(|(_, label, text)| (label, text))
        .collect()
}

#[test]
fn alias_spelling_without_semantic_evidence_keeps_the_general_cast_reading() {
    let mut project = project();
    for function in ["lock_tick", "renamed", "chained", "shadowed_rhs"] {
        assert!(casts(&flow(&mut project, function)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_resolves_numeric_aliases_by_definition_and_preserves_written_names() {
    let mut project = project();
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, label, explanation) in [
        ("lock_tick", "Convert f64 to Tick (i64)", "toward zero"),
        (
            "renamed",
            "Convert Float (f64) to Renamed (i64)",
            "toward zero",
        ),
        (
            "chained",
            "Convert f64 to state::Count (i64)",
            "toward zero",
        ),
        (
            "annotated",
            "Convert Float (f64) to Tick (i64)",
            "NaN becomes 0",
        ),
        (
            "via_call",
            "Convert Float (f64) to Tick (i64)",
            "toward zero",
        ),
        (
            "via_field",
            "Convert Float (f64) to Tick (i64)",
            "toward zero",
        ),
        (
            "shadowed_rhs",
            "Convert f64 to state::shadow::Small (u8)",
            "0 through 255",
        ),
    ] {
        let graph = flow(&mut project, function);
        let cards = casts(&graph);
        let [(heading, text)] = cards.as_slice() else {
            panic!("{function} expected one cast, got {:?}", graph.reading);
        };
        assert_eq!(*heading, label);
        assert!(text.contains(explanation), "{text}");
    }

    let nested = flow(&mut project, "via_nested");
    assert_eq!(
        casts(&nested)
            .into_iter()
            .map(|(label, _)| label)
            .collect::<Vec<_>>(),
        ["Convert f64 to Tick (i64)", "Convert Tick (i64) to u16"],
    );

    for function in [
        "generic_target",
        "conditional_target",
        "conditional_module_target",
        "associated_target",
        "nominal_source",
        "newtype_field",
        "pointer_target",
    ] {
        let graph = flow(&mut project, function);
        assert!(casts(&graph).is_empty(), "{function}: {:?}", graph.reading);
    }
}
