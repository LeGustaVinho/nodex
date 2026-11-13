use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/arc-value/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn arc_card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.first().is_some_and(|branch| branch.label.starts_with("Only owner, no Weak") || branch.label == "Only Arc owner")))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_arc_access_distinguishes_reusing_a_value_cloning_and_weak_pointers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/arc-value");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("burst", "Arc::make_mut(&mut bp)", true),
        ("entries_mut", "Arc::make_mut(&mut self.0)", true),
        ("into_entries", "Arc::unwrap_or_clone(self.0)", false),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| arc_card(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, mutable), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = arc_card(&graph).unwrap_or_else(|| panic!("Missing {name} Arc card: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { branches, .. } = &site.content else { unreachable!() };
        if mutable {
            assert_eq!(branches.len(), 3);
            assert!(branches[0].action.contains("Reuse the existing value"));
            assert_eq!(branches[1].output, "Mutable reference to the clone");
            assert!(branches[2].action.contains("Weak pointers can no longer be upgraded"));
            assert_eq!(branches[2].output, "Mutable reference to the value");
        } else {
            assert_eq!(branches.len(), 2);
            assert_eq!(branches[0].output, "Original value");
            assert!(branches[0].action.contains("Weak pointers do not prevent this"));
            assert_eq!(branches[1].output, "Cloned value");
        }
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }

    let clone = flow(&mut project, "keep_arc");
    assert!(clone.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Share ownership")));
    for name in ["get_mut", "try_unwrap", "rc_mut", "rc_owned", "local_mut", "local_owned"] {
        assert!(arc_card(&flow(&mut project, name)).is_none(), "Unexpected Arc reading in {name}");
    }
}
