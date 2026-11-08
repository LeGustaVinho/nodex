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
    let caught = global::caught_by_edges(&ctx, &sites, &handlers, &infos, &HashSet::new(), &HashSet::new());
    let local = caught.iter().find(|e| e.from == cancel_order && e.to == cancel).expect("catch (OrderNotFoundException) in Cancel");
    assert!(local.confidence.is_none());
    let program = graph.symbols.iter().find(|s| s.id.starts_with("Program.<Main>$")).expect("program").id.clone();
    let global = caught.iter().find(|e| e.from == place && e.to == program).expect("uncaught at a route: app.UseExceptionHandler");
    assert_eq!(global.confidence, Some(Confidence::Inferred));
}

#[test]
fn nestjs_routes_di_tokens_and_global_prefix() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let k = project.knowledge();
    let controller = "backend-nest/src/invoices/invoices.controller.InvoicesController";
    let find = k.routes.iter().find(|r| r.handler == format!("{controller}.findOne")).expect("@Get(\":id\") under @Controller(\"invoices\")");
    assert_eq!((find.method.as_str(), find.path.as_str(), find.normalized.as_str()), ("GET", "/api/invoices/:id", "/api/invoices/{}"), "setGlobalPrefix(\"api\")");
    let pay = k.routes.iter().find(|r| r.handler == format!("{controller}.pay")).expect("@Post");
    assert_eq!(pay.method, "POST");
    let info = |id: &str| k.symbols.iter().find(|s| s.symbol == id).unwrap_or_else(|| panic!("info {id}"));
    assert!(info(&pay.handler).capabilities.iter().any(|c| c == "AUTH_REQUIRED"), "@UseGuards");
    assert_eq!(info("backend-nest/src/main.bootstrap").entry.as_ref().map(|e| e.kind), Some(EntryKind::ServerStart));
    assert!(info("backend-nest/src/invoices/invoices.service.InvoicesService.pay").capabilities.iter().any(|c| c == "DB_WRITE"), "prisma update");
    // `{ provide: "PAYMENT_GATEWAY", useClass: StripeGateway }` + `@Inject("PAYMENT_GATEWAY") gateway: PaymentGateway`.
    let di = edge(&graph, "backend-nest/src/payments/payment.PaymentGateway", "backend-nest/src/payments/payment.StripeGateway", StructEdgeKind::ResolvesDi)
        .expect("token registration");
    assert!(di.evidence.as_deref().is_some_and(|e| e.contains("PAYMENT_GATEWAY")));
}

#[test]
fn front_end_requests_link_to_back_end_routes() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let get = edge(&graph, "frontend-react/src/api/orders.getOrders", &id_of(&graph, "OrdersController", "GetOrders"), StructEdgeKind::HttpCalls)
        .expect("fetch(\"/api/orders\") reaches [HttpGet] GetOrders in C#");
    assert_eq!(get.confidence, Some(Confidence::Inferred), "a URL match is not proven by a compiler");
    assert!(get.site.as_ref().is_some_and(|s| s.file == "frontend-react/src/api/orders.ts"));
    let cancel = edge(&graph, "frontend-react/src/api/orders.cancelOrder", &id_of(&graph, "OrdersController", "Cancel"), StructEdgeKind::HttpCalls)
        .expect("axios.post(`/api/orders/${id}/cancel`) reaches [HttpPost(\"{id}/cancel\")]");
    assert!(cancel.evidence.as_deref().is_some_and(|e| e.contains("/api/Orders/{id}/cancel")));
    // `${API_BASE}/profile` matches no route; the Stripe request goes to another host.
    assert!(!graph.edges.iter().any(|e| e.kind == StructEdgeKind::HttpCalls && (e.from.ends_with("getProfile") || e.from.ends_with("charge"))));
    let k = project.knowledge();
    assert!(k.http_calls.iter().any(|h| h.client == "axios" && h.method == "POST"));
    assert!(k.config.iter().any(|c| c.key == "VITE_API_URL" && c.source == "env"));
}

#[test]
fn javascript_tests_and_express_routes() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let k = project.knowledge();
    let case = k.symbols.iter().find(|s| s.symbol.contains("auth.test") && s.entry.as_ref().is_some_and(|e| e.kind == EntryKind::Test)).expect("it(...) callback");
    assert!(case.test);
    assert_eq!(case.entry.as_ref().map(|e| e.label.as_str()), Some("test: returns a token on login"));
    let module = edge(&graph, "ns:frontend-react/src/__tests__/auth.test", "ns:frontend-react/src/utils/auth", StructEdgeKind::Tests).expect("auth.test → auth");
    assert_eq!(module.confidence, Some(Confidence::Inferred));

    let mut express = open("ts/Frameworks");
    express.structure();
    let k = express.knowledge();
    let users = k.routes.iter().find(|r| r.normalized == "/users/{}").expect("app.get(\"/users/:id\", ...)");
    assert_eq!((users.method.as_str(), users.framework.as_str()), ("GET", "express"));
    assert_eq!(users.handler, "src/node/server.(module).GET /users/:id", "the callback's own Blueprint handles it");
    assert!(k.http_calls.iter().any(|h| h.url == "/api/heroes" && h.client == "http"), "Angular HttpClient");
}

#[test]
fn rust_routes_models_env_and_tests() {
    let mut project = open("knowledge/rust");
    let graph = project.structure();
    let k = project.knowledge();
    // axum: `.route("/orders", get(list_orders).post(create_order))`.
    let route = |method: &str, path: &str| k.routes.iter().find(|r| r.method == method && r.path == path).map(|r| (r.handler.as_str(), r.framework.as_str()));
    assert_eq!(route("GET", "/orders"), Some(("crate::api::list_orders", "axum")));
    assert_eq!(route("POST", "/orders"), Some(("crate::api::create_order", "axum")));
    assert_eq!(route("GET", "/orders/:id"), Some(("crate::api::show_order", "axum")));

    // diesel: entities with their table, keys and the write that changes one.
    let invoice = &k.models.iter().find(|m| m.symbol == "crate::db::Invoice").expect("#[derive(Insertable)]").model;
    assert_eq!((invoice.framework.as_str(), invoice.table.as_deref()), ("diesel", Some("invoices")));
    let field = |name: &str| invoice.fields.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("field {name}"));
    assert!(field("id").primary_key);
    assert_eq!(field("customer_id").target.as_deref(), Some("Customer"));
    assert!(edge(&graph, "crate::db::store", "crate::db::Invoice", StructEdgeKind::Mutates).is_some(), "diesel::insert_into(invoices::table)");

    assert!(k.config.iter().any(|c| c.key == "SHOP_API_URL" && c.owner == "crate::services::api_base"));
    assert!(k.http_calls.iter().any(|h| h.method == "POST" && h.owner == "crate::services::sync_remote"));
    let info = |id: &str| k.symbols.iter().find(|s| s.symbol == id).unwrap_or_else(|| panic!("info {id}"));
    assert_eq!(info("crate::main").entry.as_ref().map(|e| e.kind), Some(EntryKind::Main));
    assert!(info("crate::services::tests::describes_orders").test);
    assert!(info("crate::tests::checkout_test::checkout_empty_order_fails").test);
}

// ---------------------------------------------------------------------------------------------
// Review fixture: one case per knowledge bug found in review (`fixtures/knowledge/review`).

fn routes_of<'a>(k: &'a nodex_ir::knowledge::ProjectKnowledge, handler: &str) -> Vec<(&'a str, &'a str)> {
    let mut out: Vec<(&str, &str)> = k.routes.iter().filter(|r| r.handler == handler).map(|r| (r.method.as_str(), r.path.as_str())).collect();
    out.sort();
    out
}

#[test]
fn aspnet_action_routes_combine_route_and_verb_attributes() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let k = project.knowledge();
    // `[Route("api/[controller]/[action]")]` on the controller.
    assert_eq!(routes_of(&k, &id_of(&graph, "ItemsController", "List")), vec![("GET", "/api/Items/List")]);
    // `[HttpGet] [Route("{id}")]` is one route.
    assert_eq!(routes_of(&k, &id_of(&graph, "WidgetsController", "Get")), vec![("GET", "/api/widgets/{id}")]);
    // A verb with its own template keeps it; the bare verb takes the `[Route]` template.
    assert_eq!(
        routes_of(&k, &id_of(&graph, "WidgetsController", "Change")),
        vec![("POST", "/api/widgets/{id}/archive"), ("PUT", "/api/widgets/{id}/state")]
    );
    // `app.MapGet("/api/health", HealthHandlers.Check)` with the handler in another file.
    assert_eq!(routes_of(&k, &id_of(&graph, "HealthHandlers", "Check")), vec![("GET", "/api/health")]);
}

#[test]
fn a_throw_caught_in_its_own_function_reaches_no_handler() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let program = graph.symbols.iter().find(|s| s.id.starts_with("Program.<Main>$")).expect("program").id.clone();
    let remove = id_of(&graph, "WidgetsController", "Remove");
    assert!(edge(&graph, &remove, "ext:InvalidOperationException", StructEdgeKind::Throws).is_some(), "it still throws");
    assert!(
        !graph.edges.iter().any(|e| e.from == remove && e.kind == StructEdgeKind::CaughtBy),
        "caught by its own catch (InvalidOperationException)"
    );
    let rename = id_of(&graph, "WidgetsController", "Rename");
    assert!(edge(&graph, &rename, &program, StructEdgeKind::CaughtBy).is_some(), "uncaught at a route: app.UseExceptionHandler");
}

#[test]
fn a_registration_repeated_is_still_one_implementation() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let symbols: HashMap<&str, &nodex_ir::Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut children: HashMap<&str, Vec<&nodex_ir::Symbol>> = HashMap::new();
    for s in &graph.symbols {
        if let Some(p) = &s.parent {
            children.entry(p.as_str()).or_default().push(s);
        }
    }
    let types = TypeLookup::new(&graph);
    let caller = id_of(&graph, "WidgetsController", "Get");
    let iface_do = id_of(&graph, "IThing", "Do");
    let mut edges = graph.edges.clone();
    edges.push(nodex_ir::StructEdge::new(caller.clone(), iface_do, StructEdgeKind::Calls));
    let ctx = global::Ctx { symbols: &symbols, children: &children, types: &types, edges: &edges };
    let files: Vec<&nodex_ir::FileIr> = project.files().map(|(_, f)| f).collect();
    let di = global::di_edges(&ctx, &files);
    assert_eq!(di.iter().filter(|e| e.from == "Review.IThing").count(), 2, "AddScoped<IThing, Thing>() twice");
    let dispatch = global::di_dispatch(&ctx, &di);
    let thing_do = id_of(&graph, ".Thing.", "Do");
    let d = dispatch.iter().find(|e| e.from == caller && e.to == thing_do).expect("dispatched to Thing.Do");
    assert_eq!(d.confidence, Some(Confidence::Inferred));
}

#[test]
fn an_axios_instance_named_api_is_a_client_not_a_router() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let k = project.knowledge();
    assert!(!k.routes.iter().any(|r| r.span.file == "web/src/client.ts"), "no routes in the front end: {:?}", k.routes);
    let load = "web/src/client.loadWidget";
    let call = k.http_calls.iter().find(|h| h.owner == load).expect("api.get(...) is an HTTP call");
    assert_eq!((call.method.as_str(), call.url.as_str()), ("GET", "/api/widgets/${id}"));
    let get = id_of(&graph, "WidgetsController", "Get");
    assert_eq!(edge(&graph, load, &get, StructEdgeKind::HttpCalls).and_then(|e| e.confidence), Some(Confidence::Inferred));
    assert!(k.http_calls.iter().any(|h| h.owner == "web/src/client.list"), "a config object after the URL");
}

#[test]
fn route_handlers_declared_in_another_file_are_resolved() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let k = project.knowledge();
    // `app.get("/api/catalog", widgets.list)` with `import * as widgets from "./widgets.controller"`;
    // `web/src/client.list` has the same name.
    assert_eq!(routes_of(&k, "server/src/widgets.controller.list"), vec![("GET", "/api/catalog")]);
    // axum: `get(handlers::list)`.
    let list = id_of(&graph, "handlers", "list");
    assert_eq!(routes_of(&k, &list), vec![("GET", "/api/parts")]);
}

#[test]
fn global_prefixes_apply_within_their_own_package() {
    let mut project = open("knowledge/review");
    let k = project.knowledge();
    assert_eq!(routes_of(&k, "nest-a/src/reports.controller.ReportsController.all"), vec![("GET", "/alpha/reports")]);
    assert_eq!(routes_of(&k, "nest-b/src/users.controller.UsersController.all"), vec![("GET", "/beta/users")]);
}

#[test]
fn diesel_writes_without_the_crate_prefix_change_their_model() {
    let mut project = open("knowledge/review");
    let graph = project.structure();
    let gadget = id_of(&graph, "models", "Gadget");
    for f in ["add", "rename"] {
        let id = id_of(&graph, "models", f);
        assert!(edge(&graph, &id, &gadget, StructEdgeKind::Mutates).is_some(), "{f} writes gadgets");
    }
}

#[test]
fn a_qualifier_no_owner_matches_keeps_the_frontend_guess() {
    let mut project = open("knowledge/review");
    let k = project.knowledge();
    let ping = k.routes.iter().find(|r| r.path == "/api/ping").expect("app.get(\"/api/ping\", controller.ping)");
    assert_ne!(ping.handler, "web/src/client.ping", "`controller` is a package import, not web/src/client");
}

#[test]
fn fastify_route_options_name_their_handler() {
    let mut project = open("knowledge/review");
    let k = project.knowledge();
    assert_eq!(routes_of(&k, "server/src/fast.health"), vec![("GET", "/api/fast")]);
}

#[test]
fn a_library_of_controllers_takes_the_only_global_prefix() {
    let mut project = open("knowledge/nest-libs");
    let k = project.knowledge();
    assert_eq!(routes_of(&k, "libs/orders/src/orders.controller.OrdersController.list"), vec![("GET", "/api/orders")]);
}

fn info<'a>(k: &'a nodex_ir::knowledge::ProjectKnowledge, symbol: &str) -> &'a nodex_ir::knowledge::SymbolInfo {
    k.symbols.iter().find(|i| i.symbol == symbol).unwrap_or_else(|| panic!("no info for {symbol}"))
}

fn paths(k: &nodex_ir::knowledge::ProjectKnowledge) -> Vec<String> {
    k.routes.iter().map(|r| format!("{} {}", r.method, r.path)).collect()
}

#[test]
fn aspnet_authorization_combines_class_action_and_endpoint_rules() {
    let mut project = open("knowledge/auth");
    project.structure();
    let k = project.knowledge();
    let reports = info(&k, "Auth.Controllers.AdminController.Reports()").auth.clone().expect("class [Authorize]");
    assert_eq!(reports.roles, vec!["Admin", "Ops"]);
    let purge = info(&k, "Auth.Controllers.AdminController.Purge(bool)").auth.clone().expect("action policy");
    assert_eq!((purge.roles.len(), purge.policies.clone()), (2, vec!["CanPurge".to_owned()]));
    let health = info(&k, "Auth.Controllers.AdminController.Health()");
    assert!(health.auth.as_ref().is_some_and(|a| a.anonymous && a.roles.is_empty()), "[AllowAnonymous] wins");
    assert!(!health.capabilities.iter().any(|c| c == "AUTH_REQUIRED"));
    // Minimal APIs: the group's policy, plus a role required on one endpoint.
    let status = info(&k, "Handlers.Status()").auth.clone().expect("group policy");
    assert_eq!(status.policies, vec!["Operators"]);
    let restart = info(&k, "Handlers.Restart()").auth.clone().expect("endpoint role");
    assert_eq!((restart.roles.clone(), restart.policies.clone()), (vec!["Admin".to_owned()], vec!["Operators".to_owned()]));
    // An inline endpoint has no Blueprint: the program is not marked anonymous.
    assert!(info(&k, "Program.<Main>$(string[])").auth.is_none());
}

#[test]
fn nest_and_express_authorization() {
    let mut project = open("knowledge/auth");
    project.structure();
    let k = project.knowledge();
    let list = info(&k, "nest/src/users.controller.UsersController.list").auth.clone().expect("class guards");
    assert_eq!(list.policies, vec!["JwtAuthGuard", "RolesGuard"]);
    let remove = info(&k, "nest/src/users.controller.UsersController.remove").auth.clone().expect("roles");
    assert_eq!(remove.roles, vec!["admin"]);
    let public = info(&k, "nest/src/users.controller.UsersController.info");
    assert!(public.auth.as_ref().is_some_and(|a| a.anonymous));
    assert!(!public.capabilities.iter().any(|c| c == "AUTH_REQUIRED"), "@Public() is open");
    // `router.use(requireAuth)` protects the router's routes; a role middleware names its role.
    let get = info(&k, "express/src/routes/orders.(module).GET /:id").auth.clone().expect("router middleware");
    assert_eq!(get.policies, vec!["requireAuth"]);
    let purge = info(&k, "express/src/app.(module).DELETE /cache").auth.clone().expect("route middleware");
    assert_eq!(purge.roles, vec!["admin"]);
}

#[test]
fn express_mounts_prefix_imported_and_local_routers() {
    let mut project = open("knowledge/auth");
    project.structure();
    let k = project.knowledge();
    let all = paths(&k);
    for want in ["GET /api/orders", "GET /api/orders/:id", "DELETE /admin/cache"] {
        assert!(all.iter().any(|p| p == want), "{want} in {all:?}");
    }
    assert!(!all.iter().any(|p| p == "GET /:id"), "the mount prefix replaces the bare path: {all:?}");
}

#[test]
fn constructor_injection_links_consumers_to_services() {
    let mut project = open("knowledge/auth");
    let graph = project.structure();
    let dep = |from: &str, to: &str| edge(&graph, from, to, StructEdgeKind::DependsOn);
    let controller = dep("Auth.Controllers.AdminController", "Auth.Services.IReportService").expect("controller depends on its service");
    assert!(controller.confidence.is_none(), "built and provided by the container: exact");
    assert!(controller.evidence.as_deref().is_some_and(|e| e.contains("constructor")));
    assert!(dep("Auth.Services.ReportService", "Auth.Services.IClock").is_some_and(|e| e.confidence.is_none()));
    // Not registered, so maybe not built by the container (primary constructor).
    assert_eq!(dep("Auth.Services.Formatter", "Auth.Services.IClock").and_then(|e| e.confidence), Some(Confidence::Inferred));
    // Nest providers inject their constructor parameter types.
    assert!(dep("nest/src/users.controller.UsersController", "nest/src/users.controller.UsersService").is_some());
    assert!(dep("nest/src/users.controller.UsersService", "nest/src/users.controller.AuditLog").is_some());
}

#[test]
fn commands_jobs_and_message_consumers_are_entry_points() {
    let mut project = open("knowledge/cli-jobs");
    project.structure();
    let k = project.knowledge();
    let entry = |s: &str| info(&k, s).entry.clone().unwrap_or_else(|| panic!("{s} is an entry"));
    let check = |s: &str, kind: EntryKind, label: &str| {
        let e = entry(s);
        assert_eq!((e.kind, e.label.as_str()), (kind, label), "{s}");
    };
    // C#: System.CommandLine, BackgroundService, Hangfire, MassTransit.
    check("Jobs.Program.ImportFile()", EntryKind::Command, "command import");
    check("Jobs.Program.Main(string[]).SetHandler callback", EntryKind::Command, "command report");
    check("Jobs.CleanupWorker.ExecuteAsync(CancellationToken)", EntryKind::Job, "background service");
    check("Jobs.IStockReport.Send()", EntryKind::Job, "recurring job nightly-stock");
    check("Jobs.OrderPlacedConsumer.Consume(ConsumeContext<OrderPlaced>)", EntryKind::Message, "consumes OrderPlaced");
    // TypeScript: commander, setInterval, node-cron, KafkaJS, amqplib.
    check("node/src/cli.addItem", EntryKind::Command, "command add");
    check("node/src/cli.(module).action callback", EntryKind::Command, "command list");
    check("node/src/cli.heartbeat", EntryKind::Job, "every 5000 ms");
    check("node/src/cli.(module).schedule callback", EntryKind::Job, "cron 0 3 * * *");
    check("node/src/cli.handleOrder", EntryKind::Message, "Kafka consumer (eachMessage)");
    check("node/src/cli.listen.consume callback", EntryKind::Message, "queue invoices");
    // Rust: clap subcommands, kebab-cased.
    check("rust::Commands::MigrateDb", EntryKind::Command, "command migrate-db");
    check("rust::Commands::Serve", EntryKind::Command, "command serve");
    // `main` stays the program entry.
    assert_eq!(entry("rust::main").kind, EntryKind::Main);
}

#[test]
fn rust_nested_routers_take_their_mount_prefix() {
    let mut project = open("knowledge/cli-jobs");
    project.structure();
    let k = project.knowledge();
    let all = paths(&k);
    for want in ["GET /api/health", "GET /api/orders", "GET /api/orders/:id", "GET /v1/users", "GET /version", "GET /shop/items"] {
        assert!(all.iter().any(|p| p == want), "{want} in {all:?}");
    }
}

#[test]
fn ui_handlers_and_pages_are_ui_entries() {
    let mut project = open("knowledge/uml-states");
    project.structure();
    let k = project.knowledge();
    let pages: Vec<(&str, &str)> = k.routes.iter().filter(|r| r.method == "PAGE").map(|r| (r.path.as_str(), r.handler.as_str())).collect();
    assert!(pages.contains(&("/orders", "web/src/App.OrdersPage")), "{pages:?}");
    assert!(pages.contains(&("/settings", "web/src/App.SettingsPage")));
    assert!(pages.contains(&("/invoices", "web/src/pages/invoices.InvoicesPage")), "Next.js pages/ file");
    let page = info(&k, "web/src/App.OrdersPage").entry.clone().expect("page entry");
    assert_eq!((page.kind, page.label.as_str()), (EntryKind::Ui, "page /orders"));
    let click = info(&k, "web/src/App.OrdersPage.saveOrder").entry.clone().expect("click entry");
    assert_eq!((click.kind, click.label.as_str()), (EntryKind::Ui, "click \"Save order\""));
}

#[test]
fn declared_state_machines_are_read_from_their_libraries() {
    let mut project = open("knowledge/uml-states");
    project.structure();
    let k = project.knowledge();
    let machine = |framework: &str| k.state_machines.iter().find(|m| m.framework == framework).unwrap_or_else(|| panic!("{framework} machine"));
    let has = |m: &nodex_ir::knowledge::StateMachineFact, from: &str, to: &str, trigger: &str| {
        m.transitions.iter().any(|t| t.from == from && t.to == to && t.trigger.as_deref() == Some(trigger))
    };

    let x = machine("xstate");
    assert_eq!((x.name.as_str(), x.initial.as_deref()), ("fetch", Some("idle")));
    assert!(x.states.contains(&"success.stale".to_owned()), "nested states: {:?}", x.states);
    assert!(has(x, "idle", "loading", "FETCH"));
    assert!(has(x, "loading", "success", "done") && has(x, "loading", "failure", "error"), "invoke results");
    assert!(has(x, "success.fresh", "success.stale", "after 60000"), "delayed transition between siblings");
    assert!(has(x, "success.stale", "loading", "REFRESH"), "#id target");
    assert!(x.transitions.iter().any(|t| t.from == "failure" && t.guard.as_deref() == Some("canRetry")));

    let door = machine("stateless");
    assert_eq!((door.owner.as_str(), door.initial.as_deref()), ("States.Door", Some("Closed")));
    assert!(door.states.contains(&"Closed.Locked".to_owned()), "SubstateOf nests: {:?}", door.states);
    assert!(has(door, "Closed", "Closed.Locked", "Lock"));
    assert!(door.transitions.iter().any(|t| t.trigger.as_deref() == Some("Lock") && t.guard.as_deref().is_some_and(|g| g.contains("HasKey"))));
    assert!(has(door, "Closed.Locked", "Broken", "Kick"));

    let anim = machine("unity-animator");
    assert_eq!(anim.owner, "PlayerAnimation");
    assert!(anim.triggers.contains(&"Jump".to_owned()) && anim.triggers.contains(&"Grounded".to_owned()));
    assert!(has(anim, "*", "Death", "Die") && has(anim, "*", "Idle", "Respawn"));
}

/// `_state.Next(this)` is the State pattern's own method, not `Random.Next()`.
#[test]
fn a_state_objects_next_is_not_a_random_number() {
    let mut project = open("knowledge/uml-states");
    let tick = project.flow("States.TrafficLight.Tick()").expect("Tick has a body");
    let next = tick.nodes.iter().filter_map(|n| n.expr.as_ref()).find(|e| e.label == "Next").expect("the call");
    assert_eq!(next.op, None);
}
