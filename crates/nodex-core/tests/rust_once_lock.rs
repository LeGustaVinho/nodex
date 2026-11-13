use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/once-lock/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/once-lock");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, function: &str) -> FlowGraph {
    let structure = project.structure();
    let symbol = structure
        .symbols
        .iter()
        .find(|symbol| symbol.name == function && symbol.has_flow)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn cards(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph
        .reading
        .iter()
        .flatten()
        .filter(|site| {
            matches!(&site.content,
                ReadingContent::Paths { input, .. } if [
                    "Take the contents of the cell",
                    "Try to borrow the contents of",
                    "Read or initialize",
                ].iter().any(|prefix| input.starts_with(prefix))
            )
        })
        .collect()
}

#[test]
fn once_lock_spelling_alone_keeps_the_method_generic() {
    let mut project = project();
    for name in ["moved", "index", "remove_index", "edit_if_ready"] {
        assert!(cards(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_once_lock_state_changes_and_return_values() {
    let mut project = project();
    let before = flow(&mut project, "moved");
    let mutable_before = flow(&mut project, "edit_if_ready");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = flow(&mut project, "moved");
    let removed = flow(&mut project, "remove_index");
    let index = flow(&mut project, "index");
    let mutable = flow(&mut project, "edit_if_ready");
    for (graph, method, expected_outputs) in [
        (
            &after,
            "take",
            &["Discarded Some(previous value)", "Discarded None"][..],
        ),
        (&removed, "take", &["Some(previous value)", "None"][..]),
        (
            &mutable,
            "get_mut",
            &["Some(mutable reference)", "None"][..],
        ),
        (
            &index,
            "get_or_init",
            &[
                "Shared reference",
                "Shared reference to the stored result",
                "Shared reference to the stored result",
                "No value is returned",
            ][..],
        ),
    ] {
        let cards = cards(graph);
        assert_eq!(
            cards.len(),
            if graph.symbol == after.symbol { 2 } else { 1 },
            "{:?}",
            graph.reading
        );
        for card in cards {
            assert_eq!(card.basis, ValueBasis::TypeAnalysis);
            assert_eq!(
                &SOURCE[card.span.start_byte as usize..card.span.end_byte as usize],
                method
            );
            assert!(card.borrow_sources.is_none());
            let ReadingContent::Paths { branches, .. } = &card.content else {
                panic!("{card:?}")
            };
            assert!(branches.iter().all(|branch| branch.pin.is_none()));
            assert_eq!(
                branches
                    .iter()
                    .map(|branch| branch.output.as_str())
                    .collect::<Vec<_>>(),
                expected_outputs
            );
            match method {
                "take" => {
                    assert!(branches[0].action.contains("Move the stored value out"));
                    assert!(branches.iter().all(|branch| {
                        branch.action.contains("leave the cell uninitialized")
                            || branch.action.contains("Leave the cell uninitialized")
                    }));
                }
                "get_mut" => assert!(
                    branches[0]
                        .action
                        .contains("Borrow the stored value mutably")
                ),
                _ => {
                    assert!(branches[0].action.contains("skip this initializer"));
                    assert_eq!(branches[1].label, "This initializer returns");
                    assert!(branches[2].action.contains("waiting if needed"));
                    assert!(branches[3].action.contains("leaves the cell uninitialized"));
                }
            }
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
    assert_eq!(execution(&mutable_before), execution(&mutable));
    assert!(cards(&flow(&mut project, "local_calls")).is_empty());

    let structure = project.structure();
    let initializer = structure
        .symbols
        .iter()
        .find(|symbol| symbol.parent.as_ref() == Some(&index.symbol) && symbol.has_flow)
        .unwrap();
    let initializer = project.flow(&initializer.id).unwrap();
    assert!(initializer.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Paths { timing: Some(timing), .. } if timing == "If this call runs the initializer"
    )), "{:?}", initializer.reading);
}
