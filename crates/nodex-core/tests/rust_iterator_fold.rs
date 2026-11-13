use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, FlowGraph, Language, Span, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/iterator-fold/src/lib.rs");
const FOLD: &str = "Accumulate a value across iterator items";
const ENTRY: &str = "Update the accumulator for one item";

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn children(project: &mut Project, graph: &FlowGraph) -> Vec<FlowGraph> {
    let ids: Vec<_> = project.structure().symbols.iter().filter(|symbol| symbol.parent.as_ref() == Some(&graph.symbol) && symbol.has_flow)
        .map(|symbol| symbol.id.clone()).collect();
    ids.iter().map(|id| project.flow(id).unwrap()).collect()
}

fn fold(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Steps { label, .. } if label == FOLD))
}

fn entry(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == ENTRY))
}

fn snippet(span: &Span) -> &'static str {
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_fold_explains_the_seed_callback_and_final_accumulator() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/iterator-fold");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("speed_multiplier", "1.0", "js_max", None),
        ("travel", "0.0", "f64::max", None),
        ("combat", "0.0", "|sum, kind|", Some("|sum, kind|")),
        ("qualified", "4", "|total, value|", Some("|total, value|")),
        ("fallible_accumulator", "Ok(0)", "|total: Result<u32, ()>, value|", Some("|total: Result<u32, ()>, value|")),
        ("owned", "0", "|total, value|", None),
    ];
    assert!(cases.iter().all(|(name, ..)| fold(&flow(&mut project, name)).is_none()));

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for (name, seed, callback, parameters) in cases {
        let graph = flow(&mut project, name);
        let site = fold(&graph).unwrap_or_else(|| panic!("Missing fold reading for {name}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(snippet(&site.span), "fold");
        assert!(site.path.is_some());
        let ReadingContent::Steps { steps, .. } = &site.content else { unreachable!() };
        assert_eq!(snippet(&steps[0].span), seed);
        assert!(snippet(&steps[1].span).starts_with(callback));
        assert_eq!(steps[1].timing, "For each item during this call");
        assert!(steps[1].action.contains("accumulator first and the item second"));
        assert!(steps[1].action.contains("return value becomes the next accumulator"));
        assert!(steps[2].action.contains("return the starting value without calling the function"));
        assert!(entry(&graph).is_none());
        if let Some(parameters) = parameters {
            let nested = children(&mut project, &graph);
            let entries: Vec<_> = nested.iter().filter_map(|body| entry(body).map(|entry| (body, entry))).collect();
            let [(body, entry)] = entries.as_slice() else { panic!("Missing fold callback Entry for {name}: {nested:?}") };
            assert_eq!(snippet(&entry.span), parameters);
            assert_eq!(entry.path, None);
            assert!(body.nodes.iter().any(|node| node.id == entry.node && node.concept == Concept::Entry));
            if name == "fallible_accumulator" {
                assert!(body.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Paths { timing: Some(timing), branches, .. }
                    if timing == "While fold processes items" && branches[1].action.contains("Return from this callback") && !branches[1].action.contains("fold stops"))));
            }
        }
    }
    for name in ["parallel", "custom"] {
        let graph = flow(&mut project, name);
        assert!(fold(&graph).is_none());
        assert!(children(&mut project, &graph).iter().all(|body| entry(body).is_none()));
    }
    let seed = flow(&mut project, "closure_seed");
    assert!(fold(&seed).is_some());
    assert!(children(&mut project, &seed).iter().all(|body| entry(body).is_none()));
}
