//! Knowledge graph tests: facts, derived edges and project-wide summaries.

use std::path::PathBuf;
use std::sync::Arc;

use std::collections::{BTreeMap, HashMap, HashSet};

use nodex_core::Project;
use nodex_core::knowledge::{TypeLookup, flows, global};
use nodex_ir::knowledge::{Confidence, EntryKind};
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
    let ship = id_of(&graph, ".OrderService.", "ShipAsync");
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
    let ship = id_of(&graph, ".OrderService.", "ShipAsync");
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

#[test]
fn aspnet_routes_di_config_and_handlers() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let k = project.knowledge();
    let route = |method: &str, normalized: &str| k.routes.iter().find(|r| r.method == method && r.normalized == normalized);
    let get = route("GET", "/api/orders").expect("controller [HttpGet] under [Route(\"api/[controller]\")]");
    assert_eq!(get.path, "/api/Orders");
    assert_eq!(get.handler, id_of(&graph, "OrdersController", "GetOrders"));
    assert_eq!(get.framework, "aspnet");
    assert_eq!(route("POST", "/api/orders/{}/cancel").map(|r| r.handler.as_str()), Some(id_of(&graph, "OrdersController", "Cancel").as_str()));
    assert!(route("POST", "/api/orders").is_some());
    assert_eq!(route("GET", "/api/health").map(|r| r.framework.as_str()), Some("aspnet-minimal"));

    // DI registrations become exact ResolvesDi edges.
    let di = edge(&graph, "Shop.Services.IOrderService", "Shop.Services.OrderService", StructEdgeKind::ResolvesDi).expect("AddScoped<IOrderService, OrderService>");
    assert!(di.confidence.is_none());
    assert!(di.site.as_ref().is_some_and(|s| s.file.ends_with("Program.cs")));
    assert!(edge(&graph, "Shop.Services.IClock", "Shop.Services.SystemClock", StructEdgeKind::ResolvesDi).is_some());
    assert!(k.di.iter().any(|d| d.service == "AppDbContext" && d.lifetime == "context"));

    let keys: Vec<&str> = k.config.iter().map(|c| c.key.as_str()).collect();
    for key in ["ConnectionStrings:Default", "Jwt:Key", "SHOP_REGION"] {
        assert!(keys.contains(&key), "config key {key}: {keys:?}");
    }

    let info = |id: &str| k.symbols.iter().find(|s| s.symbol == id).unwrap_or_else(|| panic!("info for {id}"));
    let program = graph.symbols.iter().find(|s| s.id.starts_with("Program.<Main>$")).expect("top-level program");
    assert_eq!(info(&program.id).entry.as_ref().map(|e| e.kind), Some(EntryKind::Main), "Main beats server start");
    assert!(info(&program.id).capabilities.iter().any(|c| c == "CONFIG_READ"));
    let get_orders = info(&get.handler);
    assert_eq!(get_orders.entry.as_ref().map(|e| e.kind), Some(EntryKind::Route));
    assert!(get_orders.capabilities.iter().any(|c| c == "AUTH_REQUIRED"), "[Authorize] on the controller");
}

#[test]
fn efcore_models_writes_and_tests() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let k = project.knowledge();
    let model = |id: &str| k.models.iter().find(|m| m.symbol == id).map(|m| &m.model).unwrap_or_else(|| panic!("model {id}"));
    let order = model("Shop.Models.Order");
    assert_eq!(order.framework, "efcore");
    let field = |name: &str| order.fields.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("field {name}"));
    assert!(field("Id").primary_key);
    assert!(field("CustomerId").foreign_key);
    assert_eq!(field("CustomerId").target.as_deref(), Some("Customer"));
    assert_eq!(field("Items").target.as_deref(), Some("OrderItem"));
    model("Shop.Models.Customer");
    model("Shop.Models.OrderItem");
    assert!(k.models.iter().all(|m| m.symbol != "Shop.Models.OrderStatus"), "an enum is not an entity");

    let place = id_of(&graph, ".OrderService.", "PlaceOrderAsync");
    let writes = edge(&graph, &place, "Shop.Models.Order", StructEdgeKind::Mutates).expect("_db.Orders.Add(order)");
    assert!(writes.evidence.as_deref().is_some_and(|e| e.contains("_db.Orders")));
    let caps = &k.symbols.iter().find(|s| s.symbol == place).expect("info").capabilities;
    assert!(caps.iter().any(|c| c == "DB_WRITE"));
    let list = id_of(&graph, ".OrderService.", "ListOrdersAsync");
    assert!(k.symbols.iter().any(|s| s.symbol == list && s.capabilities.iter().any(|c| c == "DB_READ")));

    // Throws resolve to the project's exception type.
    let cancel = id_of(&graph, ".OrderService.", "CancelOrderAsync");
    assert!(edge(&graph, &cancel, "Shop.Exceptions.OrderNotFoundException", StructEdgeKind::Throws).is_some());

    // xUnit: the [Fact] is a test entry; its class is named after the service it tests.
    let fact = id_of(&graph, "OrderServiceTests", "PlaceOrder_RejectsEmptyOrders");
    assert!(k.symbols.iter().any(|s| s.symbol == fact && s.test && s.entry.as_ref().is_some_and(|e| e.kind == EntryKind::Test)));
    let tests = edge(&graph, "Shop.Tests.OrderServiceTests", "Shop.Services.OrderService", StructEdgeKind::Tests).expect("named after it");
    assert_eq!(tests.confidence, Some(Confidence::Inferred));
}

/// Call sites normally come from the Roslyn sidecar; here they are given by hand.
#[test]
fn exceptions_are_caught_by_callers_or_global_handlers() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let k = project.knowledge();
    let symbols: HashMap<&str, &nodex_ir::Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut children: HashMap<&str, Vec<&nodex_ir::Symbol>> = HashMap::new();
    for s in &graph.symbols {
        if let Some(p) = &s.parent {
            children.entry(p.as_str()).or_default().push(s);
        }
    }
    let types = TypeLookup::new(&graph);
    let ctx = global::Ctx { symbols: &symbols, children: &children, types: &types, edges: &graph.edges };
    let handlers: Vec<_> = project.files().flat_map(|(_, f)| f.knowledge.handlers.clone()).collect();
    let infos: BTreeMap<_, _> = k.symbols.iter().map(|s| (s.symbol.clone(), s.clone())).collect();
    let (cancel, cancel_order) = (id_of(&graph, "OrdersController", "Cancel"), id_of(&graph, ".OrderService.", "CancelOrderAsync"));
    let (create, place) = (id_of(&graph, "OrdersController", "Create"), id_of(&graph, ".OrderService.", "PlaceOrderAsync"));
    let sites = vec![
        global::CallSite { owner: cancel.clone(), target: cancel_order.clone(), catches: vec!["OrderNotFoundException".into()] },
        global::CallSite { owner: create.clone(), target: place.clone(), catches: Vec::new() },
    ];
    let caught = global::caught_by_edges(&ctx, &sites, &handlers, &infos, &HashSet::new());
    let local = caught.iter().find(|e| e.from == cancel_order && e.to == cancel).expect("catch (OrderNotFoundException) in Cancel");
    assert!(local.confidence.is_none());
    let program = graph.symbols.iter().find(|s| s.id.starts_with("Program.<Main>$")).expect("program").id.clone();
    let global = caught.iter().find(|e| e.from == place && e.to == program).expect("uncaught at a route: app.UseExceptionHandler");
    assert_eq!(global.confidence, Some(Confidence::Inferred));
}
