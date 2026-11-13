use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-predicates/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn predicate(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.first().is_some_and(|branch| branch.output == "The predicate's bool result")))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_option_predicates_explain_absence_without_creating_execution_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-predicates");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("expired", "field.probe.as_ref().is_some_and(|probe| probe.hp <= 0.0)", "false", Concept::Return),
        ("advance_pulses", "field.probe.as_ref().is_none_or(|probe| probe.hp <= 0.0)", "true", Concept::Branch),
        ("qualified", "Option::is_none_or(probe, test)", "true", Concept::Return),
    ];
    let before = cases.map(|(name, _, _, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| predicate(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, absent, concept), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = predicate(&graph).unwrap_or_else(|| panic!("Missing {name} predicate: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { branches, .. } = &site.content else { unreachable!() };
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].action, "Call the predicate with the value inside Some");
        assert_eq!(branches[1].action, "Skip calling the predicate");
        assert_eq!(branches[1].output, absent);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));

        let node = graph.node(&site.node).unwrap();
        assert_eq!(node.concept, concept);
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
        if name == "advance_pulses" {
            assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id && edge.from_pin == "true"
                && graph.node(&edge.to).unwrap().concept == Concept::Continue));
        }
        if name == "expired" {
            assert!(graph.reading.iter().flatten().any(|entry| entry.node == node.id && matches!(&entry.content,
                ReadingContent::Paths { branches, .. } if branches.iter().any(|branch| branch.action == "Skip the right side"))));
        }
    }

    assert!(predicate(&flow(&mut project, "local_predicates")).is_none());
}
