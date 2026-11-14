use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/partition-truncate/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/partition-truncate");
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

fn card<'a>(graph: &'a FlowGraph, method: &str) -> Option<&'a ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        source(&site.span) == method
            && match &site.content {
                ReadingContent::Steps { label, .. } => {
                    label == "Select a position and partition the slice"
                }
                ReadingContent::Paths { input, .. } => input.starts_with("Shorten "),
                _ => false,
            }
    })
}

#[test]
fn syntax_alone_does_not_classify_partition_or_truncate() {
    let mut project = project();
    let graph = flow(&mut project, "near_contacts");
    assert!(card(&graph, "select_nth_unstable_by").is_none());
    assert!(card(&graph, "truncate").is_none());
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_in_place_partition_and_vector_truncation() {
    let mut project = project();
    let before = flow(&mut project, "near_contacts");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    let graph = flow(&mut project, "near_contacts");
    let site =
        card(&graph, "select_nth_unstable_by").unwrap_or_else(|| panic!("{:?}", graph.reading));
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    let ReadingContent::Steps { steps, .. } = &site.content else {
        unreachable!()
    };
    assert_eq!(source(&steps[0].span), "NEAR_CONTACT_LIMIT - 1");
    assert!(steps[0].action.contains("counting from zero"));
    assert!(steps[0].action.contains("at or beyond the length panics"));
    assert_eq!(source(&steps[1].span), "by_distance");
    assert!(
        steps[1]
            .action
            .contains("position in the comparator's sorted order")
    );
    assert!(
        steps[1]
            .action
            .contains("An inconsistent comparator can cause a panic")
    );
    assert!(steps[2].action.contains("less than or equal"));
    assert!(steps[2].action.contains("greater than or equal"));
    assert!(steps[2].action.contains("ties can change relative order"));
    assert!(
        steps[3]
            .action
            .contains("ignores the three mutable references")
    );
    assert!(steps[3].action.contains("rearrangement remains"));

    let ufcs = flow(&mut project, "partition");
    let site =
        card(&ufcs, "select_nth_unstable_by").unwrap_or_else(|| panic!("{:?}", ufcs.reading));
    let ReadingContent::Steps { steps, .. } = &site.content else {
        unreachable!()
    };
    assert!(
        steps[3]
            .action
            .contains("Return three disjoint mutable references")
    );
    assert!(
        steps[3]
            .action
            .contains("prefix, the element at the index, and the suffix")
    );

    let shorten = flow(&mut project, "shorten");
    for (graph, length) in [(&graph, "NEAR_CONTACT_LIMIT"), (&shorten, "length")] {
        let site = card(graph, "truncate").unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths {
            input, branches, ..
        } = &site.content
        else {
            unreachable!()
        };
        assert!(input.contains("keeps its allocated capacity"));
        assert!(
            branches
                .iter()
                .all(|branch| branch.pin.is_none() && branch.output == "()")
        );
        assert!(branches[0].action.contains(&format!(
            "first {length} elements in their current order and drop the rest"
        )));
        assert!(branches[1].action.contains("without growing the vector"));
    }

    let homonyms = flow(&mut project, "homonyms");
    assert!(card(&homonyms, "select_nth_unstable_by").is_none());
    assert!(card(&homonyms, "truncate").is_none());
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&graph));
}
