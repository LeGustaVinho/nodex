use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite, ReadingStep},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/future-queue/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/future-queue");
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

fn enqueues(graph: &FlowGraph) -> Vec<(&ReadingSite, &[ReadingStep])> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Steps { label, steps } = &site.content else {
                return None;
            };
            (label == "Queue a future at the back").then_some((site, steps.as_slice()))
        })
        .collect()
}

#[test]
fn queue_spelling_alone_does_not_establish_future_polling() {
    let mut project = project();
    for name in ["exchange", "gateway", "nested", "homonyms"] {
        assert!(enqueues(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_explains_ordered_queue_insertion_in_code_and_select_handler() {
    let mut project = project();
    let baseline = flow(&mut project, "gateway");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, count) in [("exchange", 2), ("gateway", 1)] {
        let graph = flow(&mut project, name);
        let enqueues = enqueues(&graph);
        assert_eq!(enqueues.len(), count, "{name}: {:?}", graph.reading);
        for (site, steps) in enqueues {
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert_eq!(source(&site.span), "push_back");
            assert!(
                steps[0]
                    .action
                    .contains("Any work done by that expression happens here")
            );
            assert!(steps[1].action.contains("without polling it"));
            assert_eq!(steps[2].timing, "When the queue is polled");
            assert!(steps[2].action.contains("progress concurrently"));
            assert!(steps[2].action.contains("outputs in queue order"));
            if name == "gateway" {
                assert_eq!(source(&steps[2].span), "replies.next()");
                assert!(steps[2].action.contains("when its branch is enabled and polled"));
                assert!(steps[2].action.contains("after a branch has already been chosen"));
                assert!(source(&steps[0].span).starts_with("if message > 2"));
                let first = graph
                    .reading
                    .iter()
                    .flatten()
                    .find(|entry| {
                        entry.node == site.node
                            && matches!(entry.content, ReadingContent::Steps { .. })
                    })
                    .unwrap();
                assert!(matches!(&first.content, ReadingContent::Steps { label, .. }
                    if label == "Wait for one eligible branch"));
                assert!(graph.nodes.iter().any(|node| node.id == site.node && node.code.starts_with("tokio::select!")));
            }
        }
    }

    for name in ["nested", "homonyms"] {
        assert!(enqueues(&flow(&mut project, name)).is_empty());
    }
    let graph = flow(&mut project, "two_queues");
    let mut links = enqueues(&graph);
    links.sort_by_key(|(site, _)| site.span.start_byte);
    assert_eq!(links.iter().map(|(_, steps)| source(&steps[2].span)).collect::<Vec<_>>(),
        ["first.next()", "Extension::next(&mut second)", "push_back"]);
    assert!(links[2].1[2].action.starts_with("Polling the queue"));
    for name in ["different_select", "wrapped"] {
        let graph = flow(&mut project, name);
        let links = enqueues(&graph);
        assert_eq!(links.len(), 1, "{name}: {:?}", graph.reading);
        assert_eq!(source(&links[0].1[2].span), "push_back");
    }
    let after = flow(&mut project, "gateway");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&baseline), execution(&after));
}
