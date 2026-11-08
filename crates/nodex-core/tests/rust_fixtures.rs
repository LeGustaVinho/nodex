//! Snapshot, coverage, stability and Rosetta tests over the Rust fixtures.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::{Project, canon};
use nodex_ir::{Access, Concept, Expr, FileIr, FlowEdgeKind, StructEdgeKind, SymbolKind};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::{RustFrontend, lower_source};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(RustFrontend)];
    Project::open(&fixtures().join(rel), frontends).expect("fixture opens")
}

fn outline_file(ir: &FileIr) -> String {
    let mut out = String::new();
    for flow in &ir.flows {
        out.push_str(&format!("== {}\n", flow.symbol));
        out.push_str(&canon::outline(flow));
    }
    out
}

fn opaque_labels(e: &Expr, out: &mut Vec<String>) {
    if e.concept == Concept::Opaque {
        out.push(format!("{} {:?}", e.label, e.flavors));
    }
    for c in &e.children {
        opaque_labels(c, out);
    }
}

#[test]
fn sample_flows_snapshot() {
    let project = open("rust/Sample");
    for (file, ir) in project.files() {
        insta::assert_snapshot!(format!("rust_sample_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
}

#[test]
fn syntax_flows_snapshot() {
    let project = open("rust/Syntax");
    for (file, ir) in project.files() {
        insta::assert_snapshot!(format!("rust_syntax_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
}

/// Every statement and sub-expression of the fixtures maps to a concept (no `Opaque`).
#[test]
fn fixtures_are_fully_mapped() {
    for rel in ["rust/Sample", "rust/Syntax"] {
        let project = open(rel);
        let files: Vec<String> = project.files().map(|(f, _)| f.to_owned()).collect();
        assert!(!files.is_empty(), "{rel}");
        for (file, ir) in project.files() {
            assert!(ir.diagnostics.is_empty(), "{file}: {:?}", ir.diagnostics);
            let mut problems = Vec::new();
            for flow in &ir.flows {
                for n in &flow.nodes {
                    if n.concept == Concept::Opaque {
                        problems.push(format!("{}: statement {} {:?}", flow.symbol, n.code, n.flavors));
                    }
                    if let Some(e) = &n.expr {
                        let mut found = Vec::new();
                        opaque_labels(e, &mut found);
                        problems.extend(found.into_iter().map(|f| format!("{}: in `{}`: {f}", flow.symbol, n.code)));
                    }
                }
            }
            assert!(problems.is_empty(), "{file}:\n{}", problems.join("\n"));
        }
    }
}

#[test]
fn syntax_fixture_covers_the_vocabulary() {
    let project = open("rust/Syntax");
    let concepts: std::collections::BTreeSet<Concept> =
        project.files().flat_map(|(_, ir)| ir.flows.iter().flat_map(|f| f.nodes.iter().map(|n| n.concept))).collect();
    for c in [
        Concept::Var, Concept::Destructure, Concept::Assign, Concept::SetMember, Concept::Branch, Concept::Match, Concept::LoopForEach,
        Concept::LoopWhile, Concept::LoopInfinite, Concept::Break, Concept::Continue, Concept::Return, Concept::Throw, Concept::Scope,
        Concept::Spawn, Concept::Wait, Concept::Await, Concept::Emit, Concept::Lambda, Concept::Call, Concept::Macro,
    ] {
        assert!(concepts.contains(&c), "no {c:?} in the syntax fixture");
    }
}

#[test]
fn sample_structure_edges() {
    let mut project = open("rust/Sample");
    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    let kind_of = |id: &str| graph.symbols.iter().find(|s| s.id == id).map(|s| s.kind);
    // Modules follow `mod` and files.
    assert_eq!(kind_of("ns:crate"), Some(SymbolKind::Module));
    assert!(has("ns:crate", "ns:crate::domain", StructEdgeKind::Contains));
    assert!(has("ns:crate::domain", "ns:crate::domain::order", StructEdgeKind::Contains));
    assert!(has("ns:crate::services::order_service", "ns:crate::domain", StructEdgeKind::Imports));
    assert!(has("ns:crate::domain::order", "pkg:std", StructEdgeKind::Imports));
    // Types, traits and impls.
    assert_eq!(kind_of("crate::domain::order::Order"), Some(SymbolKind::Struct));
    assert_eq!(kind_of("crate::domain::order::OrderStatus"), Some(SymbolKind::Enum));
    assert_eq!(kind_of("crate::domain::order::OrderEvent"), Some(SymbolKind::Union));
    assert_eq!(kind_of("crate::domain::order::Priced"), Some(SymbolKind::Trait));
    assert!(has("crate::domain::order::Order", "crate::domain::order::Priced", StructEdgeKind::Implements));
    assert!(has("crate::domain::shapes::Circle", "crate::domain::shapes::Shape", StructEdgeKind::Implements));
    assert!(has("crate::services::repository::InMemoryOrderRepository", "crate::services::repository::OrderRepository", StructEdgeKind::Implements));
    assert!(has("crate::domain::order::Order", "ext:Display", StructEdgeKind::Implements));
    assert!(has("crate::domain::order::Order", "crate::domain::order::Order::with_status", StructEdgeKind::Contains));
    // Creation.
    assert!(has("crate::main", "crate::services::repository::InMemoryOrderRepository", StructEdgeKind::Instantiates));
}

/// Rust particularities in the structure: cross-file `impl` blocks, Drop, 2024 attributes.
#[test]
fn syntax_structure_particularities() {
    let mut project = open("rust/Syntax");
    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id);
    // `impl Display for Point` in impls.rs merges into the Point declared in expressions.rs.
    assert!(sym("crate::impls::Point").is_none(), "no placeholder type left");
    assert!(has("crate::expressions::Point", "crate::impls::Point::fmt", StructEdgeKind::Contains));
    assert!(has("crate::expressions::Point", "crate::impls::Point::length", StructEdgeKind::Contains));
    assert!(has("crate::expressions::Point", "ext:Display", StructEdgeKind::Implements));
    assert_eq!(graph.symbols.iter().find(|s| s.id == "ext:Display").map(|s| s.kind), Some(SymbolKind::Trait));
    // `#[unsafe(no_mangle)]` and `impl Drop`.
    assert!(sym("crate::modern::exported").is_some_and(|s| s.flavors.iter().any(|f| f == "attr:no_mangle")));
    assert!(sym("crate::ownership::Guard").is_some_and(|s| s.flavors.iter().any(|f| f == "impl:Drop")));

    // Drop runs on its own: an event entry, and implicit drops at the end of scopes.
    let drop = project.flow("crate::ownership::Guard::drop").unwrap();
    assert_eq!(drop.nodes[0].concept, Concept::EventEntry);
    let scoped = project.flow("crate::ownership::scoped").unwrap();
    let drops: Vec<&str> = scoped.nodes.iter().filter(|n| n.flavors.iter().any(|f| f == "implicit-drop")).map(|n| n.label.as_str()).collect();
    assert_eq!(drops, ["drop _inner at the end of its block (runs Guard's drop)", "drop _outer at the end of its block (runs Guard's drop)"]);
    assert!(scoped.nodes.iter().any(|n| n.calls.iter().any(|c| c.symbol.as_deref() == Some("crate::ownership::Guard::drop"))));
    let own = project.flow("crate::ownership::ownership").unwrap();
    assert!(own.nodes.iter().any(|n| n.label.starts_with("unlock: lock")));
    assert!(!own.nodes.iter().any(|n| n.label.contains("drop guard at")), "a dropped value is not dropped again");
    let shadow: Vec<&str> = own.nodes.iter().flat_map(|n| n.outputs.iter()).filter(|p| p.id == "var:value").map(|p| p.label.as_str()).collect();
    assert_eq!(shadow, ["value", "value₂", "value₃"]);

    // A value computed by control flow draws the flow.
    let modern = project.flow("crate::modern::search").unwrap();
    assert!(modern.nodes.iter().any(|n| n.label == "found = the value the loop below gives"));
    assert!(modern.nodes.iter().any(|n| n.concept == Concept::Break && n.label.contains("(found takes it)")));
}

#[test]
fn ownership_rides_on_the_wires() {
    let ir = lower_source(
        "src/lib.rs",
        "pub fn f(name: String, count: u32, list: &mut Vec<u32>) -> String {\n\
           let copy = count;\n\
           let borrowed = &name;\n\
           list.push(copy);\n\
           take(&mut *list);\n\
           let moved = name;\n\
           moved\n\
         }\n\
         fn take(_: &mut Vec<u32>) {}\n",
    );
    let flow = ir.flows.iter().find(|f| f.symbol == "crate::f").expect("flow");
    let access = |var: &str, code_start: &str| {
        let node = flow.nodes.iter().find(|n| n.code.starts_with(code_start)).unwrap_or_else(|| panic!("{code_start}"));
        flow.edges
            .iter()
            .find(|e| e.kind == FlowEdgeKind::Data && e.to == node.id && e.to_pin == format!("use:{var}"))
            .map(|e| e.access)
            .unwrap_or_else(|| panic!("no wire for {var} into {code_start}"))
    };
    assert_eq!(access("count", "let copy"), Some(Access::Copy));
    assert_eq!(access("name", "let borrowed"), Some(Access::Borrow));
    assert_eq!(access("name", "let moved"), Some(Access::Move));
    assert_eq!(access("moved", "moved"), Some(Access::Move));
}

#[test]
fn node_ids_survive_unrelated_insertions() {
    let before = lower_source("src/a.rs", "fn f(x: i32) -> i32 {\n    let a = x + 1;\n    if a > 2 {\n        println!(\"big\");\n    }\n    a\n}\n");
    let after = lower_source(
        "src/a.rs",
        "fn f(x: i32) -> i32 {\n    let a = x + 1;\n    let unrelated = 5;\n    if a > 2 {\n        println!(\"big\");\n    }\n    a\n}\n",
    );
    let ids = |ir: &FileIr| ir.flows[0].nodes.iter().map(|n| (n.code.clone(), n.id.clone())).collect::<Vec<_>>();
    let (b, a) = (ids(&before), ids(&after));
    for pair in &b {
        assert!(a.contains(pair), "{pair:?} changed");
    }
    assert_eq!(a.len(), b.len() + 1);
}

#[test]
fn broken_code_still_lowers() {
    let ir = lower_source("src/broken.rs", "fn f() {\n    let x = ;\n    g(\n}\npub struct S { a: i32 }\n");
    assert!(!ir.diagnostics.is_empty());
    assert!(ir.symbols.iter().any(|s| s.id == "crate::broken::S"));
}

/// The Rosetta program draws the same statements and operations in Rust, C# and TypeScript.
#[test]
fn rosetta_filter_map_sum_rust() {
    let path = fixtures().join("rosetta/filter-map-sum");
    let expected = std::fs::read_to_string(path.join("expected.canon")).unwrap().replace("\r\n", "\n");
    let mut rs = lower_source("filter_map_sum.rs", &std::fs::read_to_string(path.join("filter_map_sum.rs")).unwrap());
    let mut cs = nodex_lang_csharp::lower_source("filter_map_sum.cs", &std::fs::read_to_string(path.join("filter_map_sum.cs")).unwrap());
    assert_eq!(canon::canonical(&rs.flows[0]).trim(), expected.trim());
    nodex_core::operations::recognize_flow(&mut rs.flows[0]);
    nodex_core::operations::recognize_flow(&mut cs.flows[0]);
    assert_eq!(canon::operations(&rs.flows[0]), canon::operations(&cs.flows[0]));
}

/// The operations Rosetta in idiomatic Rust: same statements as C#/TypeScript; its operations
/// are recorded in `expected.rs.ops` (`format!` and `parse` differ from `+` and `parseInt`).
#[test]
fn rosetta_operations_rust() {
    let path = fixtures().join("rosetta/operations");
    let mut rs = lower_source("operations.rs", &std::fs::read_to_string(path.join("operations.rs")).unwrap());
    let mut cs = nodex_lang_csharp::lower_source("operations.cs", &std::fs::read_to_string(path.join("operations.cs")).unwrap());
    let rs = &mut rs.flows[0];
    let cs = &mut cs.flows[0];
    nodex_core::operations::recognize_flow(rs);
    nodex_core::operations::recognize_flow(cs);
    assert_eq!(canon::canonical(rs), canon::canonical(cs), "same statements");
    let expected = std::fs::read_to_string(path.join("expected.rs.ops")).unwrap().replace("\r\n", "\n");
    assert_eq!(canon::operations(rs).trim(), expected.trim());
}
