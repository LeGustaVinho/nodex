use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/result-collect/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/result-collect");
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

fn result_step(graph: &FlowGraph) -> Option<(&ReadingSite, &str)> {
    graph.reading.iter().flatten().find_map(|site| {
        let ReadingContent::Steps { steps, .. } = &site.content else {
            return None;
        };
        let action = &steps.last()?.action;
        action
            .contains("Stop at the first Err")
            .then_some((site, action.as_str()))
    })
}

#[test]
fn collect_spelling_alone_does_not_establish_result_behavior() {
    let mut project = project();
    for name in ["assemble", "typed", "segments", "assemble_values"] {
        assert!(result_step(&flow(&mut project, name)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer and the fixture Cargo dependencies"]
fn scip_explains_collect_into_the_standard_result_vector() {
    let mut project = project();
    let baseline = flow(&mut project, "assemble");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for name in ["assemble", "typed", "segments", "assemble_values"] {
        let graph = flow(&mut project, name);
        let (site, text) =
            result_step(&graph).unwrap_or_else(|| panic!("{name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert!(text.contains("put each Ok value into a vector"));
        assert!(text.contains("discard the partial vector, and return Err(error)"));
        assert!(text.contains("without asking for more items"));
        assert!(text.contains("Effects that already happened remain"));
        assert!(text.contains("return Ok with the vector, which can be empty"));
    }

    let after = flow(&mut project, "assemble");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&baseline), execution(&after));
    let propagation = after
        .reading
        .iter()
        .flatten()
        .find(|site| {
            site.span.file == "src/lib.rs"
                && SOURCE.get(site.span.start_byte as usize..site.span.end_byte as usize)
                    == Some("?")
        })
        .unwrap();
    assert!(
        matches!(&propagation.content, ReadingContent::Paths { branches, .. }
        if branches.iter().any(|branch| branch.pin.as_deref() == Some("error")))
    );

    for name in [
        "alias",
        "inferred",
        "separate_target",
        "parallel",
        "custom_result",
        "custom_vec",
        "custom_item",
        "custom_item_direct",
        "custom_item_by_type",
    ] {
        assert!(result_step(&flow(&mut project, name)).is_none(), "{name}");
    }
}
