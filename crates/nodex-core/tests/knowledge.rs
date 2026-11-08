//! Knowledge graph tests: facts, derived edges and project-wide summaries.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::Project;
use nodex_core::knowledge::flows;
use nodex_ir::{StructEdgeKind, StructureGraph};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::CSharpFrontend;
use nodex_lang_rust::RustFrontend;
use nodex_lang_typescript::TypeScriptFrontend;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> =
        vec![Arc::new(CSharpFrontend), Arc::new(TypeScriptFrontend), Arc::new(RustFrontend)];
    Project::open(&fixtures().join(rel), frontends).expect("fixture opens")
}

/// The id of the only symbol named `name` whose id contains `scope`.
fn id_of(graph: &StructureGraph, scope: &str, name: &str) -> String {
    let found: Vec<&str> = graph
        .symbols
        .iter()
        .filter(|s| s.name == name && s.id.contains(scope) && !s.external)
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(found.len(), 1, "one symbol {name} in {scope}: {found:?}");
    found[0].to_owned()
}

fn edge<'a>(graph: &'a StructureGraph, from: &str, to: &str, kind: StructEdgeKind) -> Option<&'a nodex_ir::StructEdge> {
    graph.edges.iter().find(|e| e.from == from && e.to == to && e.kind == kind)
}

#[test]
fn csharp_throws_link_functions_to_exception_types() {
    let mut project = open("csharp/Sample");
    let graph = project.structure();
    let ship = id_of(&graph, "OrderService", "ShipAsync");
    let e = edge(&graph, &ship, "ext:InvalidOperationException", StructEdgeKind::Throws).expect("ShipAsync throws");
    assert!(e.confidence.is_none(), "a throw is exact");
    assert!(e.site.as_ref().is_some_and(|s| s.file == "Services/OrderService.cs"));
    assert!(e.evidence.as_deref().is_some_and(|ev| ev.contains("InvalidOperationException")));
    // A throw expression (`repository ?? throw new ...`) counts too.
    let ctor = graph
        .symbols
        .iter()
        .find(|s| s.kind == nodex_ir::SymbolKind::Constructor && s.id.contains("OrderService"))
        .expect("OrderService constructor");
    assert!(edge(&graph, &ctor.id, "ext:ArgumentNullException", StructEdgeKind::Throws).is_some());
    let ext = graph.symbols.iter().find(|s| s.id == "ext:InvalidOperationException").expect("external exception type");
    assert!(ext.external);
}

#[test]
fn csharp_metrics_and_capabilities() {
    let mut project = open("csharp/Sample");
    let graph = project.structure();
    let ship = id_of(&graph, "OrderService", "ShipAsync");
    let knowledge = project.knowledge();
    let info = knowledge.symbols.iter().find(|i| i.symbol == ship).expect("ShipAsync info");
    assert!(info.complexity >= 3, "?? and if make decisions: {}", info.complexity);
    assert!(info.capabilities.iter().any(|c| c == "THROWS"));
    assert!(info.capabilities.iter().any(|c| c == "ASYNC"));
    // Straight-line code has complexity 1.
    assert!(knowledge.symbols.iter().any(|i| i.complexity == 1));
}

#[test]
fn catch_regions_cover_the_try_body() {
    let project = open("csharp/Sample");
    let main = project.flows().find(|f| f.symbol.contains("Program.Main")).expect("Main flow");
    let regions = flows::catch_regions(main);
    let ship_call = main.nodes.iter().find(|n| n.code.contains("ShipAsync(2)")).expect("call inside try");
    assert_eq!(regions.get(&ship_call.id).map(Vec::as_slice), Some(&["InvalidOperationException".to_owned()][..]));
    let outside = main.nodes.iter().find(|n| n.code.contains("Could not ship")).expect("statement inside catch");
    assert!(!regions.contains_key(&outside.id), "the catch body is not protected by its own try");
}

#[test]
fn typescript_throws_and_catch_everything() {
    let mut project = open("ts/Sample");
    let graph = project.structure();
    assert!(graph.edges.iter().any(|e| e.kind == StructEdgeKind::Throws && e.to == "ext:RangeError"));
    let main = project.flows().find(|f| f.symbol == "src/main.main").expect("main flow");
    let regions = flows::catch_regions(main);
    assert!(regions.values().any(|types| types == &["*".to_owned()]), "a JavaScript catch handles every error");
}
