use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{Concept, Expr, FlowGraph, Language, reading::ReadingContent};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn call<'a>(graph: &'a FlowGraph, method: &str) -> &'a Expr {
    graph.nodes.iter().filter_map(|node| node.expr.as_ref()).find(|expr| expr.label == method)
        .unwrap_or_else(|| panic!("Missing {method} in {}: {:?}", graph.name, graph.nodes))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_does_not_label_option_replacement_or_path_join_as_text_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/rust-operation-overloads");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let cases = [
        ("update_hit_layers", "replace", "text.replace", Some("core::option::Option::replace()")),
        ("market_groups", "join", "text.join", Some("std::path::Path::join()")),
        ("replace_text", "replace", "text.replace", None),
        ("join_text", "join", "text.join", None),
    ];
    for (name, method, operation, _) in cases {
        assert_eq!(call(&flow(&mut project, name), method).op.as_deref(), Some(operation));
    }

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    for (name, method, operation, vetoed_target) in cases {
        let graph = flow(&mut project, name);
        let expression = call(&graph, method);
        if let Some(target) = vetoed_target {
            assert_eq!(expression.target.as_deref(), Some(target));
            assert!(expression.op.is_none());
            assert_eq!(expression.concept, Concept::Call);
        } else {
            assert!(expression.target.is_some());
            assert_eq!(expression.op.as_deref(), Some(operation));
            assert_eq!(expression.concept, Concept::Operation);
        }
        if name == "update_hit_layers" {
            assert!(graph.reading.iter().flatten().any(|site| matches!(&site.content,
                ReadingContent::Paths { branches, .. } if branches[0].output == "Some(previous value)" && branches[1].output == "None")));
        }
    }
}
