use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-ok-or/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn conversion(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { input, .. }
        if input.contains("evaluate the error argument")))
}

fn snippet(site: &ReadingSite) -> &'static str {
    &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize]
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_ok_or_evaluates_its_error_argument_after_the_receiver() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-ok-or");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("fit", "catalog.get(&type_id)", "FittingError::UnknownType(type_id)", "catalog.get(&type_id).ok_or(FittingError::UnknownType(type_id))", 1),
        ("ticket", "claim?", "error()", "claim?.ok_or(error())", 2),
        ("qualified", "option", "error()", "Option::ok_or(option, error())", 0),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| conversion(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, receiver, error, call, question_marks), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = conversion(&graph).unwrap_or_else(|| panic!("Missing {name} ok_or: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(snippet(site), call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
        assert!(input.starts_with(&format!("If {receiver} produces an Option, evaluate the error argument {error} before checking")), "{input}");
        assert_eq!(branches[0].output, "Ok(value)");
        assert!(branches[0].action.ends_with("discard the evaluated error"));
        assert_eq!(branches[1].output, "Err(error)");
        assert_eq!(branches[1].action, "Use the already evaluated error");
        assert!(branches.iter().all(|branch| branch.pin.is_none()));

        let questions = graph.reading.iter().flatten().filter(|site| snippet(site) == "?");
        assert_eq!(questions.count(), question_marks);
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    let lazy = flow(&mut project, "lazy");
    assert!(conversion(&lazy).is_none());
    assert!(lazy.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.iter().any(|branch| branch.action == "Build the error with the callback"))));
    assert!(conversion(&flow(&mut project, "local_ok_or")).is_none());
}
