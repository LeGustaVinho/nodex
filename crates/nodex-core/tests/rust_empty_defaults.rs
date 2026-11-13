use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::ReadingContent};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/empty-defaults");
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

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_proves_empty_defaults_for_collection_destinations_and_wrapped_values() {
    let mut project = project();
    let before = flow(&mut project, "resolve_hit");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, expected) in [
        (
            "reconcile_locks",
            &["an empty Vec", "an empty IndexMap"][..],
        ),
        ("homonymous_vec", &["the type's default value"][..]),
        ("reference_inside", &["the type's default value"][..]),
    ] {
        let graph = flow(&mut project, function);
        let mut cards = graph
            .reading
            .iter()
            .flatten()
            .filter_map(|site| match &site.content {
                ReadingContent::Steps { label, steps }
                    if label == "Take the previous value and install the default" =>
                {
                    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
                    Some((site.span.start_byte, &steps[0].action))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        cards.sort_by_key(|(start, _)| *start);
        assert_eq!(
            cards.len(),
            expected.len(),
            "{function}: {:?}",
            graph.reading
        );
        for ((_, action), expected) in cards.iter().zip(expected) {
            assert!(
                action.contains(expected) && action.contains("Default::default()"),
                "{action}"
            );
        }
    }

    for (function, expected) in [
        ("resolve_hit", "Empty Vec"),
        ("from_result", "Empty Vec"),
        ("from_return", "Empty IndexMap"),
        ("homonymous_map", "default value"),
        ("generic", "default value"),
    ] {
        let graph = flow(&mut project, function);
        let fallback = graph
            .reading
            .iter()
            .flatten()
            .find_map(|site| match &site.content {
                ReadingContent::Paths { branches, .. }
                    if branches.len() == 2 && branches[1].action.contains("Default::default()") =>
                {
                    Some(&branches[1])
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{function}: {:?}", graph.reading));
        assert_eq!(fallback.output, expected);
        assert!(fallback.pin.is_none());
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
}
