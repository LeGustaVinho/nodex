use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/from-fn/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/from-fn");
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

fn from_fn_cards(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph
        .reading
        .iter()
        .flatten()
        .filter(|site| {
            matches!(&site.content, ReadingContent::Steps { label, .. }
                if label == "Fill an array by index" || label == "Create an iterator from a function")
        })
        .collect()
}

fn spelling(span: &Span) -> &str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

#[test]
fn from_fn_spelling_alone_keeps_the_callback_description_generic() {
    let mut project = project();
    for name in ["by_kind", "empty_array", "next_crew", "shadowed_calls"] {
        assert!(from_fn_cards(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_array_construction_from_iterator_requests() {
    let mut project = project();
    let before = flow(&mut project, "next_crew");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, call, callback) in [
        ("by_kind", "by_index(", "|kind|"),
        ("empty_array", "std::array::from_fn::<_, 0, _>(", "|_|"),
    ] {
        let graph = flow(&mut project, name);
        let cards = from_fn_cards(&graph);
        let [site] = cards.as_slice() else {
            panic!("{cards:?}")
        };
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert!(spelling(&site.span).starts_with(call));
        let ReadingContent::Steps { label, steps } = &site.content else {
            panic!("{site:?}")
        };
        assert_eq!(label, "Fill an array by index");
        assert_eq!(steps.len(), 2);
        assert!(spelling(&steps[0].span).starts_with(callback));
        assert_eq!(steps[0].timing, "During this call");
        assert!(steps[0].action.contains("index order starting at 0"));
        assert!(steps[0].action.contains("empty array requires zero calls"));
        assert!(
            steps[0]
                .action
                .contains("Each successful call supplies the element at that index")
        );
        assert_eq!(steps[1].timing, "If construction completes");
        assert_eq!(steps[1].action, "Return the completed array.");
    }

    let after = flow(&mut project, "next_crew");
    let cards = from_fn_cards(&after);
    let [site] = cards.as_slice() else {
        panic!("{cards:?}")
    };
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    assert!(spelling(&site.span).starts_with("next_with("));
    let ReadingContent::Steps { label, steps } = &site.content else {
        panic!("{site:?}")
    };
    assert_eq!(label, "Create an iterator from a function");
    assert_eq!(steps[0].timing, "During this call");
    assert!(steps[0].action.contains("store the function for later"));
    assert_eq!(steps[1].timing, "On each next() call");
    assert!(spelling(&steps[1].span).starts_with("move ||"));
    for term in [
        "Some(value)",
        "None",
        "The next request calls the function again, even after None",
        "its return value can change",
    ] {
        assert!(steps[1].action.contains(term));
    }

    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    assert!(from_fn_cards(&flow(&mut project, "shadowed_calls")).is_empty());

    for (name, expected_timing) in [
        ("array_callback", "While building the array"),
        ("iterator_callback", "On each next() call"),
    ] {
        let graph = flow(&mut project, name);
        let propagation = graph
            .reading
            .iter()
            .flatten()
            .find(|site| spelling(&site.span) == "?");
        assert!(
            matches!(propagation.map(|site| &site.content),
            Some(ReadingContent::Paths { timing: Some(timing), .. }) if timing == expected_timing),
            "{propagation:?}"
        );
    }
}
