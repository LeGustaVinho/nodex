use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    reading::{ReadingContent, ReadingSite},
    Concept, FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/vec-retain/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/vec-retain");
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
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        matches!(&site.content,
        ReadingContent::Steps { label, .. } if label == "Keep selected elements in the vector")
    })
}

fn decisions(graph: &FlowGraph) -> Vec<(&str, &str)> {
    let mut cards = graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| match &site.content {
            ReadingContent::Detail { label, .. }
                if label == "Keep this element" || label == "Remove this element" =>
            {
                assert_eq!(site.basis, ValueBasis::TypeAnalysis);
                assert!(graph.nodes.iter().any(|node| {
                    node.id == site.node && (node.concept == Concept::Return || site.path.is_some())
                }));
                Some((site.span.start_byte, (source(&site.span), label.as_str())))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    cards.sort_by_key(|(start, _)| *start);
    cards.into_iter().map(|(_, decision)| decision).collect()
}

#[test]
fn retain_spelling_alone_keeps_the_call_and_callback_generic() {
    let mut project = project();
    let graph = flow(&mut project, "advance");
    assert!(card(&graph).is_none());
    assert!(decisions(&callback(&mut project, &graph)).is_empty());
}

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_explains_vec_retention_and_literal_predicate_results() {
    let mut project = project();
    let before = flow(&mut project, "advance");
    let callback_before = callback(&mut project, &before);
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = flow(&mut project, "advance");
    let filtered = flow(&mut project, "filter");
    let forward = flow(&mut project, "forward");
    for (graph, borrow) in [(&after, "&mut T"), (&filtered, "&T"), (&forward, "&T")] {
        let card = card(graph).unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(card.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Steps { steps, .. } = &card.content else {
            panic!("{card:?}")
        };
        assert_eq!(steps[0].timing, "During the call");
        assert!(steps[0]
            .action
            .contains("once per element in its original order"));
        assert!(steps[0].action.contains(borrow));
        assert!(steps[1]
            .action
            .contains("Keep the element on true and remove it on false"));
        assert_eq!(steps[2].timing, "If the call completes");
        assert!(steps[2].action.contains("same vector"));
    }

    let callback_after = callback(&mut project, &after);
    assert_eq!(
        decisions(&callback_after),
        [
            ("false", "Remove this element"),
            ("true", "Keep this element")
        ]
    );
    assert_eq!(
        decisions(&callback(&mut project, &filtered)),
        [
            ("true", "Keep this element"),
            ("false", "Remove this element")
        ]
    );
    for name in ["explicit_tail", "nested"] {
        let graph = flow(&mut project, name);
        assert_eq!(
            decisions(&callback(&mut project, &graph)),
            [("false", "Remove this element")]
        );
    }
    for name in ["map_control", "local_control"] {
        let graph = flow(&mut project, name);
        assert!(card(&graph).is_none());
        assert!(decisions(&callback(&mut project, &graph)).is_empty());
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
    assert_eq!(execution(&callback_before), execution(&callback_after));
}
