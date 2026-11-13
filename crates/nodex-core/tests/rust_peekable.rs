use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/peekable/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn peekable_card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.first().is_some_and(|branch| matches!(branch.label.as_str(), "An item is available" | "Predicate returns true"))))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_peekable_preserves_the_next_item_when_inspected_or_rejected() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/peekable");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("replay", "inputs.next_if(|stored| stored.tick <= tick)", true),
        ("can_target", "jams.peek()", false),
        ("qualified", "Peekable::peek(values)", false),
    ];
    let before = cases.map(|(name, ..)| flow(&mut project, name));
    assert!(before.iter().all(|graph| peekable_card(graph).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, next_if), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = peekable_card(&graph).unwrap_or_else(|| panic!("Missing {name} Peekable reading: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { input, timing, branches, .. } = &site.content else { unreachable!() };
        assert!(timing.is_none());
        if next_if {
            assert!(input.contains("If an item is available, pass a reference to it"));
            assert_eq!(branches.len(), 3);
            assert_eq!(branches[0].output, "Some(item)");
            assert!(branches[1].action.contains("Keep this item as the next one"));
            assert_eq!(branches[1].output, "None");
            assert_eq!(branches[2].action, "Skip calling the predicate");
            assert_eq!(branches[2].output, "None");
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { timing: Some(timing), branches, .. }
                if timing == "Before each loop iteration" && branches[0].pin.as_deref() == Some("body") && branches[1].pin.as_deref() == Some("out"))));
        } else {
            assert!(input.contains("may pull an item from the underlying iterator and store it so it stays next"));
            assert_eq!(branches[0].output, "Some(&item)");
            assert!(branches[0].action.contains("reference to that reference"));
            assert_eq!(branches[1].label, "No item this time");
            assert_eq!(branches[1].output, "None");
        }
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert!(site.borrow_sources.is_none());
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["heap_peek", "local_peekable"] {
        assert!(peekable_card(&flow(&mut project, name)).is_none());
    }
}
