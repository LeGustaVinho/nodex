use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowGraph, Language, ValueBasis,
    reading::ReadingContent,
    semantic::ValueFactTarget,
    types::{TypeOrigin, TypeShape},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/numeric-arithmetic/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/numeric-arithmetic");
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

fn casts(graph: &FlowGraph) -> Vec<(u32, ValueBasis, &str, &str)> {
    let mut casts = graph
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
            Some((span.start_byte, site.basis, label.as_str(), text.as_str()))
        })
        .collect::<Vec<_>>();
    casts.sort_by_key(|cast| cast.0);
    casts
}

#[test]
fn arithmetic_casts_remain_generic_without_resolved_operator_evidence() {
    let mut project = project();
    for function in ["coordinate_delta", "arithmetic", "custom_delta"] {
        assert!(casts(&flow(&mut project, function)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_uses_the_concrete_numeric_operator_result_for_casts() {
    let mut project = project();
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    let facts = provider.analyze().unwrap();
    let type_at = |snippet, target| {
        let mut matches = facts.types.iter().filter(|fact| {
            let site = &fact.site;
            fact.target == target
                && site.file == "src/lib.rs"
                && SOURCE.get(site.start as usize..site.end as usize) == Some(snippet)
        });
        let info = &matches
            .next()
            .unwrap_or_else(|| panic!("missing type for {snippet}"))
            .info;
        assert!(matches.next().is_none(), "multiple types for {snippet}");
        assert_eq!(info.origin, TypeOrigin::Compiler);
        info.shape.clone()
    };
    assert_eq!(
        type_at("a - b", ValueFactTarget::Expression),
        TypeShape::scalar("Float64")
    );
    assert_eq!(
        type_at("borrowed", ValueFactTarget::Binding),
        TypeShape::Indirect {
            pointer: false,
            target: Box::new(TypeShape::scalar("Float64")),
        }
    );
    project.apply_semantic(Language::Rust, facts);

    for (function, labels) in [
        (
            "coordinate_delta",
            &["Convert f64 to i64", "Convert f64 to i64"][..],
        ),
        (
            "arithmetic",
            &[
                "Convert i16 to i64",
                "Convert i16 to i64",
                "Convert i16 to i64",
                "Convert i16 to i64",
                "Convert f64 to i64",
            ][..],
        ),
    ] {
        let graph = flow(&mut project, function);
        let cards = casts(&graph);
        let actual_labels = cards.iter().map(|card| card.2).collect::<Vec<_>>();
        assert_eq!(actual_labels, labels, "{}", graph.name);
        for (_, basis, label, text) in cards {
            assert_eq!(basis, ValueBasis::TypeAnalysis);
            if label == "Convert f64 to i64" {
                assert!(text.contains("toward zero") && text.contains("NaN becomes 0"));
            } else {
                assert!(text.contains("sign bit"));
            }
        }
    }

    let custom = flow(&mut project, "custom_delta");
    assert!(casts(&custom).is_empty(), "{:?}", custom.reading);
    assert!(custom.reading.iter().flatten().any(|site| matches!(
        &site.content,
        ReadingContent::Detail { label, text, .. }
            if label == "Resolve the operator" && text.contains("rust_numeric_arithmetic")
    )));
}
