use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Expr, FlowEdgeKind, FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/option-composition/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn composition(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.first().is_some_and(|branch| matches!(branch.label.as_str(), "Some(Some(value))" | "Both are Some"))))
}

fn expression<'a>(graph: &'a FlowGraph, operation: impl Fn(&Expr) -> bool) -> Option<&'a Expr> {
    fn locate<'a>(expr: &'a Expr, operation: &impl Fn(&Expr) -> bool) -> Option<&'a Expr> {
        if operation(expr) { return Some(expr); }
        expr.children.iter().find_map(|child| locate(child, operation))
    }
    graph.nodes.iter().filter_map(|node| node.expr.as_ref()).find_map(|expr| locate(expr, &operation))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_option_composition_from_iterator_methods() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/option-composition");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("module_practice", "local_shield.then(|| rank(\"shield-compensation\")).flatten()", true),
        ("multiple_layers", "Option::flatten(value)", true),
        ("view", "entry.module.zip(entry.source)", false),
        ("validation", "charge_volume.zip(module_capacity)", false),
    ];
    let before = cases.map(|(name, _, _)| flow(&mut project, name));
    assert!(before.iter().all(|graph| composition(graph).is_none()));
    assert!(expression(&before[0], |expr| expr.op.as_deref() == Some("list.flatten")).is_some());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for ((name, call, flatten), before) in cases.into_iter().zip(before) {
        let graph = flow(&mut project, name);
        let site = composition(&graph).unwrap_or_else(|| panic!("Missing {name} composition: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(&SOURCE[site.span.start_byte as usize..site.span.end_byte as usize], call);
        assert!(site.path.is_some());
        let ReadingContent::Paths { input, branches, .. } = &site.content else { unreachable!() };
        let paths = branches.iter().map(|branch| (branch.label.as_str(), branch.output.as_str())).collect::<Vec<_>>();
        if flatten {
            assert_eq!(paths, [("Some(Some(value))", "Some(value)"), ("Some(None)", "None"), ("None", "None")]);
            assert!(branches[0].action.contains("one layer"));
            let flatten = expression(&graph, |expr| expr.target.as_deref() == Some("core::option::Option::flatten()")).unwrap();
            assert_ne!(flatten.op.as_deref(), Some("list.flatten"));
        } else {
            assert_eq!(paths, [("Both are Some", "Some((first, second))"), ("Either is None", "None")]);
            assert!(input.contains("when both options contain a value"));
            assert!(branches[0].action.contains("receiver's value first"));
        }
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        let exec = |graph: &FlowGraph| graph.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec).cloned().collect::<Vec<_>>();
        assert_eq!(exec(&before), exec(&graph));
    }

    let iterator = flow(&mut project, "flatten_items");
    assert!(composition(&iterator).is_none());
    assert!(expression(&iterator, |expr| expr.op.as_deref() == Some("list.flatten")).is_some());
    assert!(composition(&flow(&mut project, "zip_items")).is_none());
    assert!(composition(&flow(&mut project, "local_composition")).is_none());
}
