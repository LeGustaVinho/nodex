//! Shared operations (`catalog/operations.toml`) over every fixture, and callback Blueprints.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::Project;
use nodex_core::catalog::Catalog;
use nodex_ir::{Concept, Expr, FileIr};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::CSharpFrontend;
use nodex_lang_rust::RustFrontend;
use nodex_lang_typescript::TypeScriptFrontend;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(CSharpFrontend), Arc::new(TypeScriptFrontend), Arc::new(RustFrontend)];
    Project::open(&fixtures().join(rel), frontends).expect("fixture opens")
}

fn walk<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    out.push(e);
    for c in &e.children {
        walk(c, out);
    }
}

/// Every `op` the frontends and the matcher emit exists in the catalog; pure ones are drawn as
/// `Operation` nodes and impure ones stay on the execution chain.
#[test]
fn every_emitted_operation_is_in_the_catalog() {
    let catalog = Catalog::embedded();
    let mut seen = std::collections::BTreeSet::new();
    for rel in ["csharp/Sample", "csharp/Syntax", "unity", "ts/Sample", "ts/Syntax", "ts/Frameworks", "rosetta/operations", "rust/Sample", "rust/Syntax"] {
        let mut project = open(rel);
        let symbols: Vec<String> = project.files().flat_map(|(_, ir)| ir.flows.iter().map(|f| f.symbol.clone())).collect();
        for symbol in symbols {
            let flow = project.flow(&symbol).expect("flow");
            for node in &flow.nodes {
                let mut all = Vec::new();
                if let Some(e) = &node.expr {
                    walk(e, &mut all);
                }
                for e in all {
                    let Some(op) = &e.op else { continue };
                    let info = catalog.operations.get(op).unwrap_or_else(|| panic!("{rel} {symbol}: unknown operation {op}"));
                    seen.insert(op.clone());
                    if e.concept == Concept::Operation {
                        assert!(info.pure, "{rel} {symbol}: impure {op} drawn as a pure Operation");
                    }
                    if !info.pure {
                        assert_ne!(e.concept, Concept::Operation, "{rel} {symbol}: {op}");
                    }
                }
            }
        }
    }
    // A broad sample of families shows up in the fixtures.
    for op in ["add", "less", "and", "not", "is_missing", "text.concat", "math.max", "text.length", "io.print", "list.add", "increment"] {
        assert!(seen.contains(op), "no fixture uses {op}; seen: {seen:?}");
    }
}

fn flow_names(ir: &FileIr) -> Vec<String> {
    ir.flows.iter().map(|f| f.symbol.clone()).collect()
}

#[test]
fn module_initializer_callbacks_get_blueprints_without_top_level_statements() {
    let source = r#"
        import {defineEndpoint} from "../middleware/endpoint";
        export const creatureLevelUp = defineEndpoint(
            "creatureLevelUp", { auth: true }, async (request, response) => {
                if (!request.body) { response.status(400); return; }
                await runTransaction(async (transaction) => { await transaction.save(); });
            }
        );
        const convert = register((value: number) => value + 1);
    "#;
    for suffix in ["", "\nconsole.log(creatureLevelUp);"] {
        let ir = nodex_lang_typescript::lower_source("creatures/creatureLevelUp.ts", &format!("{source}{suffix}"));
        let handlers: Vec<_> = ir.symbols.iter().filter(|s| s.flavors.iter().any(|f| f == "initializer-callback")).collect();
        assert_eq!(handlers.len(), 2);
        let handler = handlers.iter().find(|s| s.name == "defineEndpoint callback").unwrap();
        assert_eq!(handler.parent.as_deref(), Some("creatures/creatureLevelUp.(module)"));
        assert!(handler.has_flow && handler.flavors.iter().any(|f| f == "async"));
        let flow = ir.flows.iter().find(|flow| flow.symbol == handler.id).unwrap();
        assert!(flow.nodes[0].outputs.iter().any(|pin| pin.label == "request"));
        assert!(flow.nodes.iter().any(|node| node.concept == Concept::Branch));
        let names = flow_names(&ir);
        assert_eq!(names.iter().filter(|name| name.ends_with(".defineEndpoint callback")).count(), 1);
        assert!(names.iter().any(|name| name.ends_with(".defineEndpoint callback.runTransaction callback")));
    }
}

/// A block-bodied lambda handed to a call gets a Blueprint of its own; the call links to it.
#[test]
fn block_lambdas_get_their_own_blueprint() {
    let cs = nodex_lang_csharp::lower_source(
        "Callbacks.cs",
        "using System; using System.Collections.Generic;\n\
         class Callbacks {\n\
           event EventHandler Changed;\n\
           void Run(List<int> items) {\n\
             items.ForEach(x => { Console.WriteLine(x); });\n\
             items.ForEach(x => { if (x > 1) { Console.WriteLine(x * 2); } });\n\
             Changed += (s, e) => { Console.WriteLine(\"changed\"); };\n\
             items.ForEach(x => Console.WriteLine(x));\n\
           }\n\
         }\n",
    );
    let names = flow_names(&cs);
    let callback = names.iter().find(|n| n.ends_with(".ForEach callback")).unwrap_or_else(|| panic!("{names:?}"));
    assert!(names.iter().any(|n| n.ends_with(".ForEach callback~2")), "{names:?}");
    assert!(names.iter().any(|n| n.ends_with(".Changed handler")), "{names:?}");
    // The expression-bodied one stays inline.
    assert_eq!(names.iter().filter(|n| n.contains("ForEach callback")).count(), 2, "{names:?}");
    let run = cs.flows.iter().find(|f| f.symbol.ends_with("Run(List<int>)")).expect("Run");
    let call = run.nodes.iter().find(|n| n.calls.iter().any(|c| c.symbol.as_deref() == Some(callback.as_str()))).expect("linked");
    assert_eq!(call.concept, Concept::Call);
    let flow = cs.flows.iter().find(|f| &f.symbol == callback).unwrap();
    assert_eq!(flow.nodes[0].label, "when ForEach calls this function with (x)");

    let rs = nodex_lang_rust::lower_source(
        "src/callbacks.rs",
        "pub fn run(items: Vec<i32>) {
           items.iter().for_each(|x| { println!(\"{x}\"); });
           items.iter().for_each(|x| println!(\"{x}\"));
         }
",
    );
    let names = flow_names(&rs);
    assert!(names.contains(&"crate::callbacks::run::for_each callback".to_owned()), "{names:?}");
    assert_eq!(names.iter().filter(|n| n.ends_with(" callback")).count(), 1, "{names:?}");

    let ts = nodex_lang_typescript::lower_source(
        "callbacks.ts",
        "export function run(items: number[]) {\n\
           items.forEach((x) => { console.log(x); });\n\
           items.map(function (x) { return x * 2; });\n\
           items.forEach((x) => console.log(x));\n\
           items.forEach(() => {});\n\
         }\n",
    );
    let names = flow_names(&ts);
    assert!(names.contains(&"callbacks.run.forEach callback".to_owned()), "{names:?}");
    assert!(names.contains(&"callbacks.run.map callback".to_owned()), "{names:?}");
    assert_eq!(names.iter().filter(|n| n.ends_with(" callback")).count(), 2, "{names:?}");
    let run = ts.flows.iter().find(|f| f.symbol == "callbacks.run").unwrap();
    let call = run.nodes.iter().find(|n| n.calls.iter().any(|c| c.symbol.as_deref() == Some("callbacks.run.forEach callback"))).expect("linked");
    let lambda = call.expr.as_ref().unwrap().children.iter().find(|c| c.concept == Concept::Lambda).unwrap();
    assert_eq!(lambda.symbol.as_deref(), Some("callbacks.run.forEach callback"));
    assert_eq!(lambda.params, ["x"]);
}
