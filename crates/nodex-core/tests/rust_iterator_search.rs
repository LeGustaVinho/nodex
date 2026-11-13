use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/iterator-search/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/iterator-search");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let structure = project.structure();
    let symbol = structure
        .symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap();
    project.flow(&symbol.id).unwrap()
}

fn callback(project: &mut Project, owner: &FlowGraph) -> FlowGraph {
    let structure = project.structure();
    let symbol = structure
        .symbols
        .iter()
        .find(|symbol| symbol.parent.as_ref() == Some(&owner.symbol) && symbol.has_flow)
        .unwrap();
    project.flow(&symbol.id).unwrap()
}

fn source(span: &Span) -> &'static str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn site<'a>(graph: &'a FlowGraph, token: &str) -> Option<&'a ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        matches!(&site.content, ReadingContent::Paths { .. }) && source(&site.span) == token
    })
}

fn decisions(graph: &FlowGraph) -> Vec<(&str, &str)> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| match &site.content {
            ReadingContent::Detail { label, .. }
                if label == "Accept this item and stop"
                    || label == "Skip this item and keep looking" =>
            {
                assert_eq!(site.basis, ValueBasis::TypeAnalysis);
                Some((source(&site.span), label.as_str()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn search_spelling_alone_keeps_the_call_and_predicate_generic() {
    let mut project = project();
    let scan = flow(&mut project, "scan");
    assert!(site(&scan, "find").is_none());
    assert!(decisions(&callback(&mut project, &scan)).is_empty());
    assert!(site(&flow(&mut project, "locate"), "position").is_none());
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_first_match_and_the_iterator_relative_index() {
    let mut project = project();
    let before = flow(&mut project, "lookup");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, method) in [
        ("lookup", "find"),
        ("scan", "find"),
        ("scan_mut", "find"),
        ("reversed", "find"),
        ("chained", "find"),
        ("locate", "position"),
        ("after_advance", "position"),
    ] {
        let graph = flow(&mut project, function);
        let search =
            site(&graph, method).unwrap_or_else(|| panic!("{function}: {:?}", graph.reading));
        assert_eq!(search.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths {
            input,
            branches,
            deferred,
            ..
        } = &search.content
        else {
            unreachable!()
        };
        assert!(!deferred);
        assert!(input.contains("During this call"));
        assert!(input.contains("the test returns false"));
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert!(branches[0].action.contains("pick up after it"));
        assert_eq!(branches[1].output, "None");
        if method == "find" {
            assert!(input.contains("&Item") && input.contains("a reference to that reference"));
            assert_eq!(branches[0].output, "Some(item)");
        } else {
            assert!(input.contains("as Item") && input.contains("from zero at the next item"));
            assert_eq!(branches[0].output, "Some(index)");
        }
    }

    let after = flow(&mut project, "lookup");
    let propagation = site(&after, "?").unwrap();
    let ReadingContent::Paths { branches, .. } = &propagation.content else {
        unreachable!()
    };
    assert!(
        branches
            .iter()
            .any(|branch| branch.pin.as_deref() == Some("error"))
    );
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));

    for function in ["scan", "locate"] {
        let graph = flow(&mut project, function);
        let callback = callback(&mut project, &graph);
        let mut decisions = decisions(&callback);
        decisions.sort_unstable();
        assert_eq!(
            decisions,
            [
                ("false", "Skip this item and keep looking"),
                ("true", "Accept this item and stop")
            ]
        );
    }
    assert!(site(&flow(&mut project, "text"), "find").is_none());
    let custom = flow(&mut project, "custom");
    assert!(site(&custom, "find").is_none());
    assert!(decisions(&callback(&mut project, &custom)).is_empty());
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_when_any_and_all_stop_and_what_an_empty_iterator_returns() {
    let mut project = project();
    let before = flow(&mut project, "pending");
    assert!(site(&flow(&mut project, "any_active"), "any").is_none());
    assert!(site(&flow(&mut project, "all_pairs"), "all").is_none());
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, method) in [
        ("any_active", "any"),
        ("all_pairs", "all"),
        ("ascii", "all"),
        ("has_separator", "any"),
        ("all_increased", "all"),
        ("pending", "any"),
        ("after_decisive", "any"),
        ("after_decisive", "all"),
        ("empty_results", "any"),
        ("empty_results", "all"),
    ] {
        let graph = flow(&mut project, function);
        let card =
            site(&graph, method).unwrap_or_else(|| panic!("{function}: {:?}", graph.reading));
        assert_eq!(card.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths {
            input,
            branches,
            deferred,
            ..
        } = &card.content
        else {
            unreachable!()
        };
        let (decisive, fallback) = if method == "any" {
            ("true", "false")
        } else {
            ("false", "true")
        };
        assert!(!deferred);
        assert!(input.contains("During this call"));
        assert!(input.contains("by value as Item, which can itself be a reference"));
        assert!(input.contains(&format!(
            "When the test returns {fallback}, try the next item"
        )));
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert_eq!(branches[0].output, decisive);
        assert!(branches[0].action.contains("pick up after it"));
        assert_eq!(branches[1].output, fallback);
        assert!(branches[1].action.contains("yielded no items"));
    }

    let after = flow(&mut project, "pending");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    for (function, labels) in [
        (
            "any_active",
            [
                ("true", "Stop and return true from any"),
                ("false", "Keep looking for a match"),
            ],
        ),
        (
            "all_pairs",
            [
                ("false", "Stop and return false from all"),
                ("true", "Keep checking the other items"),
            ],
        ),
    ] {
        let graph = flow(&mut project, function);
        let callback = callback(&mut project, &graph);
        for (token, label) in labels {
            assert!(callback.reading.iter().flatten().any(|site| {
                source(&site.span) == token && site.basis == ValueBasis::TypeAnalysis
                    && matches!(&site.content, ReadingContent::Detail { label: actual, .. } if actual == label)
            }), "{function} callback: {:?}", callback.reading);
        }
    }
    let custom = flow(&mut project, "custom_all");
    assert!(site(&custom, "all").is_none());
    assert!(callback(&mut project, &custom).reading.iter().flatten().all(|site| {
        !matches!(&site.content, ReadingContent::Detail { label, .. } if label.contains("from all"))
    }));
}
