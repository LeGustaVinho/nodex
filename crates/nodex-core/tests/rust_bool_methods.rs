use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/bool-methods/src/lib.rs");

fn open() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/bool-methods");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn decision(flow: &FlowGraph) -> Option<&ReadingSite> {
    flow.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Paths { input, .. } if input.starts_with("Evaluate ")))
}

fn propagation(flow: &FlowGraph) -> &ReadingContent {
    &flow.reading.iter().flatten().find(|site| site.span.file == "src/lib.rs"
        && SOURCE.get(site.span.start_byte as usize..site.span.end_byte as usize) == Some("?")).unwrap().content
}

fn assignments(flow: &FlowGraph) -> Vec<(&ReadingSite, &str, &str)> {
    flow.reading.iter().flatten().filter_map(|site| {
        let ReadingContent::Detail { label, text, .. } = &site.content else { return None };
        if label != "Update a boolean with OR" && label != "Update a boolean with AND" {
            return None;
        }
        Some((site, label.as_str(), text.as_str()))
    }).collect()
}

#[test]
fn bool_method_names_alone_do_not_imply_evaluation_order() {
    let mut project = open();
    for name in ["lazy", "lazy_ufcs", "eager", "local_then", "local_then_some"] {
        assert!(decision(&flow(&mut project, name)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_the_lazy_and_eager_standard_bool_methods() {
    let mut project = open();
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for name in ["lazy", "lazy_ufcs"] {
        let flow = flow(&mut project, name);
        let site = decision(&flow).unwrap_or_else(|| panic!("Missing standard bool reading for {name}"));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
        assert!(input.starts_with("Evaluate enabled first, then evaluate the callback argument || Some(probe? + 1)"), "{input}");
        assert_eq!(branches[0].output, "Some(callback result)");
        assert_eq!(branches[1].action, "Skip calling the callback");
        assert_eq!(branches[1].output, "None");
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert!(matches!(propagation(&flow), ReadingContent::Paths { deferred: true, timing: Some(timing), branches, .. }
            if timing == "When the bool is true" && branches[1].action == "Return from this callback"));
        assert!(flow.nodes.iter().all(|node| node.outputs.iter().all(|pin| pin.id != "error")));
    }

    let eager = flow(&mut project, "eager");
    let site = decision(&eager).unwrap();
    let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
    assert!(input.starts_with("Evaluate false first, then evaluate probe? even when the bool is false"), "{input}");
    assert_eq!(branches[0].output, "Some(value)");
    assert_eq!(branches[1].action, "Discard the evaluated argument");
    assert_eq!(branches[1].output, "None");
    assert!(branches.iter().all(|branch| branch.pin.is_none()));
    assert!(matches!(propagation(&eager), ReadingContent::Paths { deferred: false, timing: None, branches, .. }
        if branches[1].action == "Return from eager" && branches[1].pin.as_deref() == Some("error")));

    let local = flow(&mut project, "local_then");
    assert!(decision(&local).is_none());
    assert!(matches!(propagation(&local), ReadingContent::Paths { deferred: true, timing: None, .. }));
    assert!(decision(&flow(&mut project, "local_then_some")).is_none());
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_eager_primitive_boolean_assignment_and_its_store() {
    let mut project = open();
    let before = flow(&mut project, "assign_boolean");
    let early_before = flow(&mut project, "assign_with_early_exit");
    assert!(assignments(&before).is_empty());
    assert!(assignments(&early_before).is_empty());
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    let after = flow(&mut project, "assign_boolean");
    let mut cards = assignments(&after);
    cards.sort_by_key(|(site, ..)| site.span.start_byte);
    assert_eq!(cards.len(), 2, "{:?}", after.reading);
    for ((site, label, text), (token, operation, decisive, condition)) in cards.iter().zip([
        ("|=", "OR", "true", "either boolean"),
        ("&=", "AND", "false", "both booleans"),
    ]) {
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(site.span.file, "src/lib.rs");
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], token);
        assert_eq!(*label, format!("Update a boolean with {operation}"));
        assert!(text.contains(&format!("right-hand expression even when the destination is already {decisive}")));
        assert!(text.contains(&format!("store true in the destination when {condition}")));
        assert!(text.contains("If evaluation completes"));
    }

    let early = flow(&mut project, "assign_with_early_exit");
    assert_eq!(assignments(&early).len(), 1);
    assert!(matches!(propagation(&early), ReadingContent::Paths { branches, .. }
        if branches[1].pin.as_deref() == Some("error") && branches[0].output.contains("|=")));
    let execution = |graph: &FlowGraph| {
        graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    assert_eq!(execution(&early_before), execution(&early));

    for name in ["assign_integer", "assign_by_reference", "assign_custom", "lazy_operators"] {
        let graph = flow(&mut project, name);
        assert!(assignments(&graph).is_empty());
        if name != "lazy_operators" {
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content,
                ReadingContent::Detail { label, .. } if label == "Resolve the operator")));
        }
    }
}
