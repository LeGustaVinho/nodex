use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/tokio-oneshot-reading/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn oneshot(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph.reading.iter().flatten().filter(|site| match &site.content {
        ReadingContent::Detail { label, .. } => label == "Create a channel for one value",
        ReadingContent::Paths { branches, .. } => branches.first().is_some_and(|branch|
            matches!(branch.label.as_str(), "The channel accepts the value" | "The channel delivers a value")),
        _ => false,
    }).collect()
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_oneshot_channel_send_and_both_ways_of_waiting() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/tokio-oneshot-reading");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [("durable", vec!["reply_channel", "await"]), ("durable_blocking", vec!["blocking_recv"]), ("publish", vec!["send"])];
    let before = cases.each_ref().map(|(name, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| oneshot(graph).is_empty()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, expected), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let mut sites = oneshot(&graph);
        sites.sort_by_key(|site| site.span.start_byte);
        assert_eq!(sites.len(), expected.len(), "{name}: {:?}", graph.reading);
        for (site, token) in sites.into_iter().zip(expected) {
            assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], token);
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            if let ReadingContent::Paths { input, branches, .. } = &site.content {
                assert!(branches.iter().all(|branch| branch.pin.is_none()));
                match token {
                    "await" => {
                        assert!(input.contains("receiver is itself a future") && input.contains("task can suspend"));
                        assert_eq!(branches[1].output, "Err(RecvError)");
                    }
                    "blocking_recv" => {
                        assert!(input.contains("blocking the current thread"));
                        assert_eq!(branches[1].output, "Err(RecvError)");
                    }
                    "send" => {
                        assert!(input.contains("Use up this sender") && input.contains("false"));
                        assert_eq!(branches[0].output, "Ok(())");
                        assert_eq!(branches[1].output, "Err(value)");
                    }
                    _ => unreachable!(),
                }
            } else {
                assert_eq!(token, "reply_channel");
            }
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["homonyms", "unrelated"] {
        assert!(oneshot(&flow(&mut project, name)).is_empty());
    }
    assert!(flow(&mut project, "unrelated").reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Await a result")));
}
