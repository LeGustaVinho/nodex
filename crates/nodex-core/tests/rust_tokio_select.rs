use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/tokio-select/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn selection(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Steps { label, .. } if label == "Wait for one eligible branch"))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_select_explains_direct_loop_handlers_without_execution_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/tokio-select");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let before = flow(&mut project, "pilot");
    assert!(selection(&before).is_none());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let pilot = flow(&mut project, "pilot");
    let selection_steps = |site: &ReadingSite| match &site.content {
        ReadingContent::Steps { steps, .. } => steps.clone(),
        _ => unreachable!(),
    };
    let site = selection(&pilot).unwrap();
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    let steps = selection_steps(site);
    assert_eq!(steps.len(), 3, "{steps:?}");
    assert_eq!(steps[0].action, "socket_next() → run this branch");
    for (step, keyword, action) in [
        (&steps[1], "continue", "send_tick() → Skip the rest of this iteration and start the next one."),
        (&steps[2], "break", "sleep_until(deadline) → Leave the enclosing loop."),
    ] {
        assert_eq!(&SOURCE[step.span.start_byte as usize..step.span.end_byte as usize], keyword);
        assert!(step.action.starts_with(action), "{step:?}");
        assert!(step.action.ends_with("This select! supplies no value on this path."), "{step:?}");
    }
    let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
    assert_eq!(exec(&before), exec(&pilot));

    let complex = flow(&mut project, "complex_handlers");
    let steps = selection_steps(selection(&complex).unwrap());
    assert!(steps[0].action.ends_with("run this branch"));
    assert!(steps[1].action.ends_with("run this branch"));
    assert!(steps[2].action.starts_with("Leave the enclosing loop."));

    assert!(selection(&flow(&mut project, "local_select")).is_none());
}
