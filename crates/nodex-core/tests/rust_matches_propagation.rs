use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/matches-propagation/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn readings(graph: &FlowGraph) -> Vec<&ReadingSite> {
    graph.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Paths { input, .. }
        if input == "Evaluate the first argument of matches!")).collect()
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_reads_propagation_in_the_first_argument_of_the_standard_matches_macro() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/matches-propagation");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("specialized", vec![("\"])?", true)]),
        ("generic_and_nested", vec![("input?", false), ("input?)?", true)]),
        ("optional", vec![("*/ input?", true)]),
    ];
    let before = cases.iter().map(|(name, _)| flow(&mut project, name)).collect::<Vec<_>>();
    assert!(before.iter().all(|graph| readings(graph).is_empty()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, expected), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let mut sites = readings(&graph);
        sites.sort_by_key(|site| site.span.start_byte);
        assert_eq!(sites.len(), expected.len(), "{name}: {:?}", graph.reading);
        for (site, (marker, direct)) in sites.into_iter().zip(expected) {
            let offset = SOURCE.find(marker).unwrap() + marker.len() - 1;
            assert_eq!(site.span.start_byte as usize, offset);
            assert_eq!(site.span.end_byte as usize, offset + 1);
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert!(site.path.is_some());
            let ReadingContent::Paths { branches, .. } = &site.content else { unreachable!() };
            assert_eq!(branches[0].action, if name == "optional" { "Take the value inside Some" } else { "Take the value inside Ok" });
            assert_eq!(branches[0].output, if direct { "Test the pattern with this value" } else { "Continue with the value" });
            assert_eq!(branches[1].output, "This pattern test produces no bool");
            assert!(branches[1].action.contains(&format!("Return early from {name}")));
            assert!(branches.iter().all(|branch| branch.pin.is_none()));
        }
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }
    for name in ["guard_only", "nested_closure", "literal_and_comment", "local_macro", "wrapped_macro"] {
        assert!(readings(&flow(&mut project, name)).is_empty(), "{name}");
    }
}
