use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite, ReadingStep},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/vec-cleanup/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/vec-cleanup");
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

fn card<'a>(graph: &'a FlowGraph, method: &str) -> Option<(&'a ReadingSite, &'a [ReadingStep])> {
    graph.reading.iter().flatten().find_map(|site| {
        let ReadingContent::Steps { label, steps } = &site.content else {
            return None;
        };
        (source(&site.span) == method
            && matches!(
                label.as_str(),
                "Remove an element by swapping in the last one"
                    | "Remove an element and preserve the order"
                    | "Remove consecutive duplicates"
            ))
        .then_some((site, steps.as_slice()))
    })
}

#[test]
fn syntax_alone_does_not_establish_vector_removal_behavior() {
    let mut project = project();
    for (name, method) in [
        ("module_profile", "swap_remove"),
        ("move_crew", "remove"),
        ("sorted_unique", "dedup"),
    ] {
        assert!(card(&flow(&mut project, name), method).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_swapping_shifting_and_adjacent_deduplication() {
    let mut project = project();
    let before = flow(&mut project, "module_profile");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, method) in [
        ("module_profile", "swap_remove"),
        ("take", "swap_remove"),
        ("move_crew", "remove"),
        ("remove_ufcs", "remove"),
    ] {
        let graph = flow(&mut project, name);
        let (site, steps) =
            card(&graph, method).unwrap_or_else(|| panic!("{name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(source(&steps[0].span), "index");
        assert!(steps[0].action.contains("outside the vector panics"));
        assert!(
            steps
                .last()
                .unwrap()
                .action
                .contains("Return the removed element by value")
        );
        if method == "swap_remove" {
            assert!(steps[1].action.contains("Move the last element"));
            assert!(steps[1].action.contains("can change the order"));
            assert_eq!(steps[2].timing, "If the index is the last element");
            assert!(steps[2].action.contains("No other element needs to move"));
        } else {
            assert!(steps[1].action.contains("left by one slot"));
            assert!(steps[1].action.contains("keep their relative order"));
        }
    }

    for name in ["sorted_unique", "dedup_ufcs"] {
        let graph = flow(&mut project, name);
        let (site, steps) =
            card(&graph, "dedup").unwrap_or_else(|| panic!("{name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert!(steps[0].action.contains("PartialEq"));
        assert!(
            steps[0]
                .action
                .contains("Keep the first element in each run of equal neighbors")
        );
        assert!(steps[0].action.contains("drop the rest in place"));
        assert!(
            steps[1]
                .action
                .contains("separated by different values can both remain")
        );
        assert!(steps[1].action.contains("returns () (unit)"));
    }

    let homonyms = flow(&mut project, "homonyms");
    for method in ["swap_remove", "remove", "dedup"] {
        assert!(card(&homonyms, method).is_none());
    }
    let after = flow(&mut project, "module_profile");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    let propagation = after
        .reading
        .iter()
        .flatten()
        .find(|site| source(&site.span) == "?")
        .unwrap();
    assert!(
        matches!(&propagation.content, ReadingContent::Paths { branches, .. }
        if branches[0].output.contains("receiver of .swap_remove"))
    );
}
