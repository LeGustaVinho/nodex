use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowGraph, Language, ValueBasis, reading::{ReadingBranch, ReadingContent}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/iterator-try/src/lib.rs");

fn open() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/iterator-try");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, function: &str) -> FlowGraph {
    let symbol = project.structure().symbols.iter().find(|symbol| symbol.name == function).unwrap().id.clone();
    project.flow(&symbol).unwrap()
}

fn callback_flow(project: &mut Project, function: &str) -> FlowGraph {
    let structure = project.structure();
    let owner = &structure.symbols.iter().find(|symbol| symbol.name == function).unwrap().id;
    let callback = structure.symbols.iter().find(|symbol| symbol.parent.as_ref() == Some(owner) && symbol.has_flow).unwrap();
    project.flow(&callback.id).unwrap()
}

struct Propagation<'a> {
    branches: &'a [ReadingBranch],
    basis: ValueBasis,
    deferred: bool,
    timing: Option<&'a str>,
    start: u32,
}

fn propagations(flow: &FlowGraph) -> Vec<Propagation<'_>> {
    flow.reading.iter().flatten().filter_map(|site| {
        let span = &site.span;
        if span.file != "src/lib.rs" || SOURCE.get(span.start_byte as usize..span.end_byte as usize) != Some("?") {
            return None;
        }
        let ReadingContent::Paths { branches, timing, deferred, .. } = &site.content else { return None };
        Some(Propagation { branches, basis: site.basis, timing: timing.as_deref(), deferred: *deferred, start: span.start_byte })
    }).collect()
}

fn assert_context(flow: &FlowGraph, action: &str, output: &str, timing: &str) {
    let paths = propagations(flow);
    let [path] = paths.as_slice() else { panic!("Expected one ? in {}, found {}", flow.name, paths.len()) };
    assert_eq!(path.basis, ValueBasis::TypeAnalysis);
    assert!(path.deferred);
    assert_eq!(path.timing, Some(timing));
    assert!(path.branches.iter().all(|branch| branch.pin.is_none()));
    assert_eq!(path.branches[0].output, "Continue this callback with the value");
    assert!(path.branches[1].action.contains(action), "{:?}", path.branches[1]);
    assert_eq!(path.branches[1].output, output);
}

#[test]
fn iterator_spelling_alone_keeps_the_generic_question_mark_explanation() {
    let mut project = open();
    for function in ["search", "search_ufcs", "keep_values", "fold_result", "local_method"] {
        let flow = flow(&mut project, function);
        let paths = propagations(&flow);
        let [path] = paths.as_slice() else { panic!("Expected one ? in {function}") };
        assert_eq!(path.basis, ValueBasis::Syntax);
        assert_eq!(path.branches[1].action, "Return from this callback");
        assert!(flow.nodes.iter().all(|node| node.outputs.iter().all(|pin| pin.id != "error")));
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_explains_which_iterator_operation_receives_the_early_callback_result() {
    let mut project = open();
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, action, output, timing, card) in [
        ("search", "find_map tries the next item", "None", "While find_map searches", "Find the first value produced by the callback"),
        ("search_ufcs", "find_map tries the next item", "None", "While find_map searches", "Find the first value produced by the callback"),
        ("keep_values", "filter_map skips this item", "None", "When the iterator requests an item", "Yield values produced by the callback"),
        ("fold_option_ufcs", "try_fold stops and returns the callback's early result", "Early callback result", "While try_fold processes items", "Accumulate until the callback stops"),
        ("fold_result", "try_fold stops and returns the callback's early result", "Early callback result", "While try_fold processes items", "Accumulate until the callback stops"),
    ] {
        let flow = flow(&mut project, function);
        assert_context(&flow, action, output, timing);
        assert!(flow.nodes.iter().all(|node| node.outputs.iter().all(|pin| pin.id != "error")));
        assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == card)));
    }
    let poll = callback_flow(&mut project, "fold_poll");
    assert_context(&poll, "try_fold stops and returns the callback's early result", "Early callback result", "While try_fold processes items");

    for (function, action, timing) in [
        ("local_method", "Return from this callback", None),
        ("nested_closure", "Return from this callback", None),
        ("nested_future", "Finish this async block", Some("When the future runs")),
        ("fold_initial_closure", "Return from this callback", None),
    ] {
        let flow = flow(&mut project, function);
        let paths = propagations(&flow);
        let [path] = paths.as_slice() else { panic!("Expected one ? in {function}: {}", paths.len()) };
        assert_eq!(path.branches[1].action, action);
        assert_eq!(path.timing, timing);
    }

    let outer = flow(&mut project, "outer_question_mark");
    let mut paths = propagations(&outer);
    paths.sort_by_key(|path| path.start);
    let [callback, caller] = paths.as_slice() else { panic!("Expected two ? operators: {}", paths.len()) };
    assert!(callback.branches[1].action.contains("find_map tries the next item"));
    assert!(callback.branches.iter().all(|branch| branch.pin.is_none()));
    assert_eq!(caller.branches[1].action, "Return from outer_question_mark");
    assert_eq!(caller.branches[1].pin.as_deref(), Some("error"));
    assert!(!caller.deferred);
    assert!(outer.nodes.iter().any(|node| node.outputs.iter().any(|pin| pin.id == "error")));
}
