use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-and/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-and");
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

fn source(span: &Span) -> &'static str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn choice(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| {
        source(&site.span) == "and" && matches!(&site.content, ReadingContent::Paths { .. })
    })
}

#[test]
fn and_spelling_alone_keeps_the_call_generic() {
    let mut project = project();
    for function in ["launcher_type_id", "choose", "qualified", "local_and"] {
        assert!(choice(&flow(&mut project, function)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_eager_option_and_and_keeps_both_question_mark_exits() {
    let mut project = project();
    let before = flow(&mut project, "choose");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = flow(&mut project, "choose");
    let qualified = flow(&mut project, "qualified");
    let launcher = flow(&mut project, "launcher_type_id");
    for graph in [&after, &qualified, &launcher] {
        let card = choice(graph).unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(card.basis, ValueBasis::TypeAnalysis);
        let ReadingContent::Paths { branches, .. } = &card.content else {
            panic!("{card:?}")
        };
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert_eq!(branches[0].output, "Second Option (Some or None)");
        assert_eq!(branches[1].output, "None");
        assert!(branches[0].action.contains("Discard the value inside Some"));
        assert!(branches[1].action.contains("Discard the already evaluated second option"));
    }

    let steps = after
        .reading
        .iter()
        .flatten()
        .find_map(|site| match &site.content {
            ReadingContent::Steps { label, steps } if label == "Evaluate both options in order" => {
                Some(steps.as_slice())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(source(&steps[0].span), "primary()?");
    assert!(steps[0].action.contains("the second option is skipped"));
    assert_eq!(source(&steps[1].span), "candidate()?");
    assert!(steps[1].action.contains("even if the first is None"));
    assert!(
        steps[1]
            .action
            .contains("leave the scope before the method runs")
    );

    let questions = after
        .reading
        .iter()
        .flatten()
        .filter(|site| source(&site.span) == "?")
        .collect::<Vec<_>>();
    assert_eq!(questions.len(), 2);
    for question in questions {
        assert!(
            matches!(&question.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.is_none()
                && branches[0].output.contains("and(...)")
                && branches[1].action == "Return from choose"
                && branches[1].pin.as_deref() == Some("error"))
        );
    }
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    assert!(choice(&flow(&mut project, "local_and")).is_none());
}
