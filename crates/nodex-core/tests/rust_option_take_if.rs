use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-take-if/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-take-if");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, function: &str) -> FlowGraph {
    let symbol = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == function)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn card(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        matches!(&site.content, ReadingContent::Paths { input, .. }
            if input.starts_with("Take the contents of"))
    })
}

#[test]
fn option_take_if_spelling_alone_keeps_the_method_generic() {
    let mut project = project();
    for name in ["normalize", "keep_edited", "local_test"] {
        assert!(card(&flow(&mut project, name)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_when_take_if_runs_the_test_and_removes_or_keeps_its_value() {
    let mut project = project();
    let before = flow(&mut project, "normalize");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = flow(&mut project, "normalize");
    let edited = flow(&mut project, "keep_edited");
    for graph in [&after, &edited] {
        let site = card(graph).unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(
            &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize],
            "take_if"
        );
        let ReadingContent::Paths { branches, .. } = &site.content else {
            panic!("{site:?}")
        };
        assert_eq!(branches.len(), 3);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert_eq!(branches[0].output, "None");
        assert!(branches[0].action.contains("Skip the test"));
        assert_eq!(branches[1].output, "Some(taken value)");
        assert!(branches[1].action.contains("mutable reference"));
        assert!(branches[1].action.contains("leave None"));
        assert_eq!(branches[2].output, "None");
        assert!(branches[2].action.contains("Leave the value in the option"));
        assert!(
            branches[2]
                .action
                .contains("including any changes made by the test")
        );
        assert!(site.borrow_sources.is_none());
    }
    let structure = project.structure();
    let callback = structure
        .symbols
        .iter()
        .find(|symbol| symbol.parent.as_ref() == Some(&edited.symbol) && symbol.has_flow)
        .unwrap();
    let callback = project.flow(&callback.id).unwrap();
    assert!(
        callback.reading.iter().flatten().any(|site| matches!(
            &site.content,
            ReadingContent::Paths { timing: Some(timing), .. }
                if timing == "When the option contains a value"
        )),
        "{:?}",
        callback.reading
    );

    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    assert!(card(&flow(&mut project, "local_test")).is_none());
}
