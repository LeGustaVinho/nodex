use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingBranch, ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-update/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-update");
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

fn source(span: &Span) -> &'static str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn paths_at<'a>(
    graph: &'a FlowGraph,
    method: &str,
) -> Option<(&'a ReadingSite, &'a [ReadingBranch])> {
    graph
        .reading
        .iter()
        .flatten()
        .find_map(|site| match &site.content {
            ReadingContent::Paths { branches, .. } if source(&site.span) == method => {
                Some((site, branches.as_slice()))
            }
            _ => None,
        })
}

#[test]
fn option_method_spelling_alone_does_not_establish_standard_behavior() {
    let mut project = project();
    for (function, method) in [
        ("stage_layers", "replace"),
        ("initialize_crew", "get_or_insert"),
        ("restore_layers", "unwrap_or_default"),
        ("read_or_default", "unwrap_or_default"),
    ] {
        assert!(paths_at(&flow(&mut project, function), method).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_option_mutation_and_the_conditional_default() {
    let mut project = project();
    let before = flow(&mut project, "restore_layers");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, method, expected) in [
        ("stage_layers", "replace", ["Some(previous value)", "None"]),
        (
            "initialize_crew",
            "get_or_insert",
            [
                "Mutable reference to the existing value",
                "Mutable reference to the inserted value",
            ],
        ),
        (
            "restore_layers",
            "unwrap_or_default",
            ["value", "default value"],
        ),
        (
            "read_or_default",
            "unwrap_or_default",
            ["value", "default value"],
        ),
    ] {
        let graph = flow(&mut project, function);
        let (site, branches) =
            paths_at(&graph, method).unwrap_or_else(|| panic!("{function}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.output.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        match method {
            "replace" => assert!(
                branches
                    .iter()
                    .all(|branch| branch.action.contains("Store the new value as Some"))
            ),
            "get_or_insert" => {
                assert!(branches[0].action.contains("Keep the existing contents"));
                assert!(
                    branches[1]
                        .action
                        .contains("Store the evaluated candidate as Some")
                );
                let evaluation = graph
                    .reading
                    .iter()
                    .flatten()
                    .find_map(|site| match &site.content {
                        ReadingContent::Steps { label, steps }
                            if label == "Evaluate the candidate before checking the option" =>
                        {
                            Some(steps.as_slice())
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(evaluation.len(), 1);
                assert_eq!(source(&evaluation[0].span), "candidate()");
                assert!(
                    evaluation[0]
                        .action
                        .contains("even if the option already contains a value")
                );
            }
            _ => {
                assert!(!branches[0].action.contains("Default::default()"));
                assert!(branches[1].action.contains("Default::default()"));
            }
        }
    }

    let after = flow(&mut project, "restore_layers");
    let (fallback, _) = paths_at(&after, "unwrap_or_default").unwrap();
    let replace = after
        .reading
        .iter()
        .flatten()
        .find(|site| {
            source(&site.span) == "replace" && matches!(&site.content, ReadingContent::Steps { .. })
        })
        .unwrap();
    assert_eq!(replace.node, fallback.node);
    assert_ne!(replace.path, fallback.path);
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));

    let local = flow(&mut project, "local_calls");
    for method in ["replace", "get_or_insert", "unwrap_or_default"] {
        assert!(paths_at(&local, method).is_none(), "{:?}", local.reading);
    }
}
