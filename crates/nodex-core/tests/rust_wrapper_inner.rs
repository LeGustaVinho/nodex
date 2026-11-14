use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingBranch, ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/wrapper-inner/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/wrapper-inner");
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

fn source(span: &Span) -> &str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn extractions(graph: &FlowGraph) -> Vec<(&ReadingSite, &str, &[ReadingBranch])> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| {
            let ReadingContent::Paths {
                input, branches, ..
            } = &site.content
            else {
                return None;
            };
            matches!(
                branches.first()?.label.as_str(),
                "The mutex is not poisoned" | "This Arc is the only strong owner" | "Initialized"
            )
            .then_some((site, input.as_str(), branches.as_slice()))
        })
        .collect()
}

#[test]
fn wrapper_spelling_alone_does_not_establish_extraction_behavior() {
    let mut project = project();
    for name in [
        "keep",
        "skill_levels",
        "mutex_ufcs",
        "once_ufcs",
        "arc_only",
        "homonyms",
        "other_std",
    ] {
        assert!(extractions(&flow(&mut project, name)).is_empty());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_the_three_standard_wrappers_at_their_own_calls() {
    let mut project = project();
    let before = flow(&mut project, "keep");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (name, count) in [
        ("keep", 3),
        ("skill_levels", 1),
        ("mutex_ufcs", 1),
        ("once_ufcs", 1),
        ("arc_only", 1),
    ] {
        let graph = flow(&mut project, name);
        let cards = extractions(&graph);
        assert_eq!(cards.len(), count, "{name}: {:?}", graph.reading);
        for (site, input, branches) in cards {
            assert_eq!(site.basis, ValueBasis::TypeAnalysis);
            assert!(branches.iter().all(|branch| branch.pin.is_none()));
            match branches[0].label.as_str() {
                "The mutex is not poisoned" => {
                    assert_eq!(source(&site.span), "into_inner");
                    assert!(input.contains("without acquiring the lock"));
                    assert_eq!(branches[0].output, "Ok(value)");
                    assert!(branches[1].output.contains("PoisonError"));
                    assert!(branches[1].action.contains("recovers the value"));
                }
                "This Arc is the only strong owner" => {
                    assert_eq!(source(&site.span), "try_unwrap");
                    assert!(input.contains("Take ownership"));
                    assert_eq!(branches[0].output, "Ok(value)");
                    assert!(branches[0].action.contains("without cloning"));
                    assert!(
                        branches[0]
                            .action
                            .contains("Weak references may still exist")
                    );
                    assert_eq!(branches[1].output, "Err(original Arc)");
                    assert!(branches[1].label.contains("during the attempt"));
                }
                "Initialized" => {
                    assert_eq!(source(&site.span), "into_inner");
                    assert!(input.contains("Consume the cell"));
                    assert_eq!(branches[0].output, "Some(value)");
                    assert_eq!(branches[1].output, "None");
                }
                _ => unreachable!(),
            }
        }
    }
    for name in ["homonyms", "other_std"] {
        assert!(extractions(&flow(&mut project, name)).is_empty());
    }
    let after = flow(&mut project, "keep");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
}
