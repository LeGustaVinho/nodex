use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-map-fallback/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-map-fallback");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let symbol = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn source(span: &Span) -> &'static str {
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn mapping(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        matches!(source(&site.span), "map_or" | "map_or_else")
            && matches!(&site.content, ReadingContent::Paths { .. })
    })
}

#[test]
fn option_map_fallback_spelling_alone_keeps_the_call_generic() {
    let mut project = project();
    for name in ["group_name", "eager", "lazy", "shadowed"] {
        assert!(mapping(&flow(&mut project, name)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_eager_values_and_the_two_conditional_callbacks() {
    let mut project = project();
    let before = flow(&mut project, "lazy");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = flow(&mut project, "lazy");
    let eager = flow(&mut project, "eager");
    let group = flow(&mut project, "group_name");
    for (graph, expected) in [
        (&after, "Fallback result"),
        (&eager, "Fallback value"),
        (&group, "Fallback value"),
    ] {
        let site = mapping(graph).unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths { branches, .. } = &site.content else {
            panic!("{site:?}")
        };
        assert_eq!(branches[0].output, "Transformation result");
        assert!(branches[0].action.contains("value inside Some"));
        assert_eq!(branches[1].output, expected);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
    }
    let steps = group
        .reading
        .iter()
        .flatten()
        .find_map(|site| match &site.content {
            ReadingContent::Steps { label, steps }
                if label == "Evaluate the arguments before checking the option" =>
            {
                Some(steps.as_slice())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(source(&steps[0].span), "String::new()");
    assert!(
        steps[0]
            .action
            .contains("even if the option contains a value")
    );
    let factories = flow(&mut project, "factories");
    let steps = factories
        .reading
        .iter()
        .flatten()
        .find_map(|site| match &site.content {
            ReadingContent::Steps { label, steps }
                if label == "Evaluate the arguments before checking the option" =>
            {
                Some(steps.as_slice())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(source(&steps[0].span), "make_fallback()");
    assert_eq!(source(&steps[1].span), "make_mapper()");
    assert_eq!(steps[1].timing, "If the fallback argument completes");

    for (graph, expected) in [
        (
            &after,
            &[
                "When the option contains a value",
                "When the option is empty",
            ][..],
        ),
        (&eager, &["When the option contains a value"][..]),
    ] {
        let structure = project.structure();
        let callbacks = structure
            .symbols
            .iter()
            .filter(|symbol| symbol.parent.as_ref() == Some(&graph.symbol) && symbol.has_flow)
            .map(|symbol| symbol.id.clone())
            .collect::<Vec<_>>();
        let mut timing = Vec::new();
        for callback in callbacks {
            let flow = project.flow(&callback).unwrap();
            timing.extend(
                flow.reading
                    .iter()
                    .flatten()
                    .filter_map(|site| match &site.content {
                        ReadingContent::Paths {
                            timing: Some(timing),
                            ..
                        } => Some(timing.clone()),
                        _ => None,
                    }),
            );
        }
        timing.sort();
        assert_eq!(timing, expected);
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
    assert!(mapping(&flow(&mut project, "shadowed")).is_none());
    assert!(mapping(&flow(&mut project, "result_control")).is_none());
}
