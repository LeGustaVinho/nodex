use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-as-deref-mut/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-as-deref-mut");
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

fn cards(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph.reading.iter().flatten().filter(|site| {
        matches!(&site.content, ReadingContent::Paths { input, .. } if input.starts_with("Borrow through "))
    }).collect()
}

#[test]
fn as_deref_mut_spelling_alone_keeps_the_call_generic() {
    let mut project = project();
    for name in ["run_fighter_phase", "text_view", "string_view", "shadowed"] {
        assert!(cards(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_borrowing_through_the_option_without_moving_it() {
    let mut project = project();
    let before = flow(&mut project, "run_fighter_phase");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    let facts = provider.analyze().unwrap();
    assert!(facts.refs.iter().any(|reference| {
        reference.target.display == "core::option::Option::as_deref_mut()"
            && SOURCE.get(reference.site.start as usize..reference.site.end as usize)
                == Some("text.as_deref_mut()")
    }));
    project.apply_semantic(Language::Rust, facts);
    let after = flow(&mut project, "run_fighter_phase");
    let text = flow(&mut project, "text_view");
    for (graph, count) in [(&after, 2), (&text, 1)] {
        let cards = cards(graph);
        assert_eq!(cards.len(), count, "{:?}", graph.reading);
        for card in cards {
            assert_eq!(card.basis, ValueBasis::TypeAnalysis);
            assert_eq!(
                &SOURCE[card.span.start_byte as usize..card.span.end_byte as usize],
                "as_deref_mut"
            );
            assert!(card.borrow_sources.is_none());
            let ReadingContent::Paths {
                input, branches, ..
            } = &card.content
            else {
                panic!("{card:?}")
            };
            assert!(input.contains("stays in place"));
            assert!(input.contains("when the borrow ends"));
            assert_eq!(branches.len(), 2);
            assert!(branches.iter().all(|branch| branch.pin.is_none()));
            assert!(branches[0].action.contains("DerefMut"));
            assert!(branches[0].action.contains("borrow its target again"));
            assert_eq!(branches[0].output, "Some(mutable reference to the target)");
            assert_eq!(branches[1].output, "None");
        }
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
    assert!(cards(&flow(&mut project, "string_view")).is_empty());
    assert!(cards(&flow(&mut project, "shadowed")).is_empty());
}
