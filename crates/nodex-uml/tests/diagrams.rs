//! UML diagrams of the fixtures. Each test checks what matters in the model and snapshots the
//! Mermaid text; the text is also written to `tests/mmd/*.mmd`, which `web/src/uml/mermaid.test.ts`
//! parses with the Mermaid version the web app ships.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_uml::{MemberFilter, PackageGrouping, Runtime, UmlDiagram, UmlInput, UmlKind, UmlOptions, UmlScope, availability, build};
use nodex_trace::CallSpan;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![
        Arc::new(nodex_lang_csharp::CSharpFrontend),
        Arc::new(nodex_lang_typescript::TypeScriptFrontend),
        Arc::new(nodex_lang_rust::RustFrontend),
    ];
    Project::open(&fixtures().join(rel), frontends).expect("fixture opens")
}

fn diagram(rel: &str, kind: UmlKind, scope: UmlScope, opts: UmlOptions) -> UmlDiagram {
    diagram_with(rel, kind, scope, opts, None)
}

fn diagram_with(rel: &str, kind: UmlKind, scope: UmlScope, opts: UmlOptions, runtime: Option<&nodex_uml::Runtime>) -> UmlDiagram {
    let mut project = open(rel);
    let graph = project.structure();
    let knowledge = project.knowledge();
    let units = project.units();
    let name = project.summary().name;
    let flows = std::sync::Mutex::new(project);
    let input = UmlInput { graph: &graph, knowledge: &knowledge, units: &units, project: &name, flows: &flows, runtime };
    build(&input, kind, &scope, &opts).expect("diagram builds")
}

/// Saves the text for the Mermaid parse test.
fn save(name: &str, d: &UmlDiagram) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mmd");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}.mmd")), &d.mermaid).unwrap();
}

fn id_of<'a>(d: &'a UmlDiagram, label: &str) -> &'a str {
    d.elements.iter().find(|e| e.label == label).map(|e| e.mermaid_id.as_str()).unwrap_or_else(|| panic!("no element {label}: {:?}", d.elements.iter().map(|e| &e.label).collect::<Vec<_>>()))
}

fn relation<'a>(d: &'a UmlDiagram, from: &str, to: &str) -> Option<&'a nodex_uml::UmlRelation> {
    let (a, b) = (id_of(d, from), id_of(d, to));
    d.relations.iter().find(|r| r.from == a && r.to == b)
}

#[test]
fn csharp_class_diagram_reads_members_relations_and_multiplicities() {
    let d = diagram("csharp/Sample", UmlKind::Class, UmlScope::Project, UmlOptions { members: MemberFilter::All, ..UmlOptions::default() });
    save("csharp_class", &d);
    // Inheritance and realization.
    assert_eq!(relation(&d, "Circle", "Shape").map(|r| r.kind.as_str()), Some("inherits"));
    assert_eq!(relation(&d, "Shape", "IEntity").map(|r| r.kind.as_str()), Some("realizes"));
    assert_eq!(relation(&d, "InMemoryOrderRepository", "IOrderRepository").map(|r| r.kind.as_str()), Some("realizes"));
    // A dictionary field: a qualified association to many orders.
    let orders = relation(&d, "InMemoryOrderRepository", "Order").expect("repository holds orders");
    assert_eq!(orders.label.as_deref(), Some("_orders [int]"));
    assert!(d.mermaid.contains("\"0..*\""), "{}", d.mermaid);
    // Members with UML visibility and parameters.
    assert!(d.mermaid.contains("+WithStatus(status: OrderStatus) Order"), "{}", d.mermaid);
    assert!(d.mermaid.contains("-_radius: double"));
    assert!(d.mermaid.contains("+Area()* double"), "abstract operation");
    assert!(d.mermaid.contains("<<enumeration>>") && d.mermaid.contains("<<interface>>"));
    insta::assert_snapshot!("csharp_class", d.mermaid);
}

#[test]
fn rust_and_typescript_class_diagrams() {
    let rs = diagram("rust/Sample", UmlKind::Class, UmlScope::Project, UmlOptions::default());
    save("rust_class", &rs);
    assert_eq!(relation(&rs, "Circle", "Shape").map(|r| r.kind.as_str()), Some("realizes"), "impl Shape for Circle");
    assert!(rs.mermaid.contains("<<trait>>"));
    // A Rust struct holds its fields by value: composition.
    assert_eq!(relation(&rs, "Order", "OrderStatus").map(|r| r.kind.as_str()), Some("composition"));

    let ts = diagram("ts/Sample", UmlKind::Class, UmlScope::Folder { path: "src/domain".into() }, UmlOptions::default());
    save("ts_class_domain", &ts);
    assert_eq!(relation(&ts, "Circle", "Shape").map(|r| r.kind.as_str()), Some("inherits"));
    assert_eq!(relation(&ts, "Order", "Entity").map(|r| r.kind.as_str()), Some("realizes"));
    assert!(ts.title.contains("src/domain"));
    insta::assert_snapshot!("ts_class_domain", ts.mermaid);
}

#[test]
fn knowledge_shapes_class_diagrams_entities_controllers_and_injection() {
    let d = diagram("knowledge/fullstack", UmlKind::Class, UmlScope::Folder { path: "backend-dotnet".into() }, UmlOptions::default());
    save("fullstack_class_dotnet", &d);
    assert!(d.mermaid.contains("<<controller>>"), "{}", d.mermaid);
    assert!(d.mermaid.contains("<<entity"), "data models are entities");
    // The controller gets its service through the constructor: aggregation.
    let injected = relation(&d, "OrdersController", "IOrderService").expect("controller uses its service");
    assert_eq!((injected.kind.as_str(), injected.label.as_deref()), ("aggregation", Some("_orderService")));
    assert!(!d.mermaid.contains("class n0[\"Program\"] {
    <<controller>>"), "a Minimal API program is not a controller");
    // Entities relate through their keys and navigations.
    assert!(d.relations.iter().any(|r| r.evidence.as_deref().is_some_and(|e| e.contains("foreign key") || e.contains("refers to"))));

    let auth = diagram("knowledge/auth", UmlKind::Class, UmlScope::Folder { path: "dotnet".into() }, UmlOptions::default());
    save("auth_class", &auth);
    let di = relation(&auth, "IReportService", "ReportService").or_else(|| relation(&auth, "ReportService", "IReportService"));
    assert!(di.is_some_and(|r| r.kind == "realizes"));
    assert!(auth.mermaid.contains("<<service scoped>>"), "{}", auth.mermaid);
}

#[test]
fn class_scopes_narrow_to_a_file_a_selection_or_a_symbol() {
    let file = diagram("csharp/Sample", UmlKind::Class, UmlScope::File { path: "Services/OrderService.cs".into() }, UmlOptions::default());
    save("csharp_class_file", &file);
    let in_scope: Vec<&str> = file.elements.iter().filter(|e| !e.context).map(|e| e.label.as_str()).collect();
    assert!(in_scope.contains(&"OrderService") && in_scope.contains(&"IOrderRepository"), "{in_scope:?}");
    assert!(!in_scope.contains(&"Circle"));
    // Types of other files the file's types use are drawn as context.
    assert!(file.elements.iter().any(|e| e.context && e.label == "Order"), "{:?}", file.elements);
    assert!(file.mermaid.contains(":::ctx"));

    let symbol = diagram("csharp/Sample", UmlKind::Class, UmlScope::Symbol { id: "Sample.Domain.Shape".into() }, UmlOptions { context_neighbors: false, ..UmlOptions::default() });
    assert_eq!(symbol.elements.len(), 1);

    let project = open("csharp/Sample");
    let span = project.flows().find(|f| f.symbol.contains("OrderService.ShipAsync")).map(|f| f.span.clone()).expect("ShipAsync");
    let range = diagram(
        "csharp/Sample",
        UmlKind::Class,
        UmlScope::Range { file: span.file.clone(), start_byte: span.start_byte + 10, end_byte: span.start_byte + 20 },
        UmlOptions { context_neighbors: false, ..UmlOptions::default() },
    );
    assert_eq!(range.elements.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), vec!["OrderService"], "the type around the selection");
}

#[test]
fn package_diagrams_nest_namespaces_and_find_cycles() {
    let d = diagram("csharp/Sample", UmlKind::Package, UmlScope::Project, UmlOptions::default());
    save("csharp_package", &d);
    let labels: Vec<&str> = d.elements.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Sample.Domain") && labels.contains(&"Sample.Services"), "{labels:?}");
    let dep = relation(&d, "Sample.Services", "Sample.Domain").expect("services use the domain");
    assert!(dep.count > 1);
    assert!(d.elements.iter().all(|e| e.detail.as_deref().is_some_and(|x| x.contains("instability"))));
    insta::assert_snapshot!("csharp_package", d.mermaid);

    let folders = diagram("ts/Sample", UmlKind::Package, UmlScope::Project, UmlOptions { grouping: PackageGrouping::Folder, ..UmlOptions::default() });
    save("ts_package", &folders);
    assert!(folders.elements.iter().any(|e| e.label == "src/domain"));

    let review = diagram("knowledge/review", UmlKind::Package, UmlScope::Project, UmlOptions::default());
    save("review_package", &review);
    for w in &review.warnings {
        assert!(!w.is_empty());
    }
}

#[test]
fn component_diagrams_connect_front_end_back_end_and_infrastructure() {
    let d = diagram("knowledge/fullstack", UmlKind::Component, UmlScope::Project, UmlOptions::default());
    save("fullstack_component", &d);
    let comps: Vec<&str> = d.elements.iter().filter(|e| e.kind == "component").map(|e| e.label.as_str()).collect();
    for want in ["backend-dotnet", "backend-nest", "frontend-react"] {
        assert!(comps.contains(&want), "{want} in {comps:?}");
    }
    assert!(!comps.iter().any(|c| c.contains("Tests")), "tests hidden by default: {comps:?}");
    // The React client calls both APIs over HTTP.
    let http: Vec<_> = d.relations.iter().filter(|r| r.kind == "http").collect();
    assert!(http.iter().any(|r| r.from == id_of(&d, "frontend-react") && r.to == id_of(&d, "backend-dotnet")), "{http:?}");
    assert!(http.iter().all(|r| r.label.as_deref().is_some_and(|l| l.contains('/'))));
    // The database with its tables.
    assert!(d.elements.iter().any(|e| e.kind == "database" && e.label.contains("EF Core")), "{:?}", d.elements);
    assert!(d.mermaid.contains("-.->"), "HTTP is dashed");
    insta::assert_snapshot!("fullstack_component", d.mermaid);

    // One project only: its folders are the layers.
    let layers = diagram("csharp/Sample", UmlKind::Component, UmlScope::Project, UmlOptions::default());
    save("csharp_component_layers", &layers);
    let names: Vec<&str> = layers.elements.iter().filter(|e| e.kind == "component").map(|e| e.label.as_str()).collect();
    assert!(names.contains(&"Services") && names.contains(&"Domain"), "{names:?}");
    assert!(layers.mermaid.contains("«layer»"));
}

#[test]
fn rust_workspace_components_follow_cargo_path_dependencies() {
    let d = diagram("knowledge/rust", UmlKind::Component, UmlScope::Project, UmlOptions::default());
    save("rust_component", &d);
    let comps: Vec<&str> = d.elements.iter().filter(|e| e.kind == "component").map(|e| e.label.as_str()).collect();
    assert!(comps.len() >= 2, "{comps:?}");
    assert!(d.elements.iter().any(|e| e.kind == "database"), "diesel tables: {:?}", d.elements);
}

#[test]
fn availability_suggests_the_most_useful_diagram() {
    let mut project = open("knowledge/fullstack");
    let graph = project.structure();
    let knowledge = project.knowledge();
    let units = project.units();
    let input = UmlInput { graph: &graph, knowledge: &knowledge, units: &units, project: "fullstack", flows: &nodex_uml::NoFlows, runtime: None };
    let a = availability(&input, &UmlScope::Project).unwrap();
    assert_eq!(a.suggested, Some(UmlKind::Component));
    assert!(a.kinds.iter().any(|k| k.kind == UmlKind::Class && k.available));
    assert!(a.kinds.iter().any(|k| k.kind == UmlKind::Sequence && k.available));
    assert!(a.kinds.iter().any(|k| k.kind == UmlKind::UseCase && k.available));
    // A route handler talks to other parts: its sequence; a private helper: its logic.
    let route = availability(&input, &UmlScope::Symbol { id: "Shop.Controllers.OrdersController.Cancel(Guid)".into() }).unwrap();
    assert_eq!(route.suggested, Some(UmlKind::Sequence));
    let helper = availability(&input, &UmlScope::Symbol { id: "Shop.Services.OrderService.Audit(Order, string)".into() }).unwrap();
    assert_eq!(helper.suggested, Some(UmlKind::Activity));
    let models = availability(&input, &UmlScope::Folder { path: "backend-dotnet/Models".into() }).unwrap();
    assert!(models.kinds.iter().any(|k| k.kind == UmlKind::Activity && !k.available), "{models:?}");
    let file = availability(&input, &UmlScope::File { path: "backend-dotnet/Models/Order.cs".into() }).unwrap();
    assert_eq!(file.suggested, Some(UmlKind::Class));
    assert!(availability(&input, &UmlScope::Symbol { id: "nope".into() }).is_err());
}

fn count(d: &UmlDiagram, kind: &str) -> usize {
    d.elements.iter().filter(|e| e.kind == kind).count()
}

/// Every activation a sequence diagram opens is closed (Mermaid rejects the text otherwise).
fn balanced(d: &UmlDiagram) {
    let opens = d.mermaid.matches("->>+").count();
    let closes = d.mermaid.matches("-->>-").count() + d.mermaid.matches("--x-").count() + d.mermaid.lines().filter(|l| l.trim_start().starts_with("deactivate ")).count();
    assert_eq!(opens, closes, "{}", d.mermaid);
}

#[test]
fn activity_diagrams_follow_branches_loops_and_errors() {
    let id = "Shop.Services.OrderService.PlaceOrderAsync(Order, int)";
    let d = diagram("knowledge/fullstack", UmlKind::Activity, UmlScope::Symbol { id: id.into() }, UmlOptions::default());
    save("fullstack_activity_place_order", &d);
    assert_eq!(d.title, "Activity of OrderService.PlaceOrderAsync");
    assert_eq!(count(&d, "decision"), 3, "two ifs and an else-if");
    assert_eq!(count(&d, "loop"), 1);
    assert_eq!(count(&d, "throw"), 1);
    // The loop body goes back to the loop; the loop exits when done.
    let lp = &d.elements.iter().find(|e| e.kind == "loop").unwrap().mermaid_id;
    assert!(d.mermaid.lines().filter(|l| l.trim_end().ends_with(&format!(" {lp}")) && !l.contains("-->|")).count() >= 2, "{}", d.mermaid);
    assert!(d.mermaid.contains(&format!("{lp} -->|\"done\"|")));
    // The throw ends the activity with its exception type; the write is annotated.
    assert!(d.mermaid.contains("-->|\"ArgumentException\"|"), "{}", d.mermaid);
    assert!(d.mermaid.contains("✎ Order"));
    // Merged actions point at the code they cover.
    assert!(d.elements.iter().filter(|e| e.kind == "action").all(|e| e.span.as_ref().is_some_and(|s| s.file.ends_with("OrderService.cs"))));
    assert!(d.candidates.first().is_some_and(|c| c.id == id));
    insta::assert_snapshot!("fullstack_activity_place_order", d.mermaid);
}

#[test]
fn activity_try_catch_selection_and_expansion() {
    let cancel = "Shop.Controllers.OrdersController.Cancel(Guid)";
    let d = diagram("knowledge/fullstack", UmlKind::Activity, UmlScope::Symbol { id: cancel.into() }, UmlOptions::default());
    save("fullstack_activity_cancel", &d);
    assert!(d.mermaid.contains("subgraph g0 [\"try\"]"), "{}", d.mermaid);
    assert!(d.mermaid.contains("-.->|\"catch OrderNotFoundException\"|"));
    assert!(d.mermaid.contains("-->|\"POST /api/Orders/{id}/cancel\"|"), "the route starts it");

    // With a depth, the service call is expanded in place, in a box named after it.
    let deep = diagram("knowledge/fullstack", UmlKind::Activity, UmlScope::Symbol { id: cancel.into() }, UmlOptions { depth: Some(1), ..UmlOptions::default() });
    save("fullstack_activity_cancel_deep", &deep);
    assert!(deep.mermaid.contains("[\"OrderService.CancelOrderAsync\"]"), "{}", deep.mermaid);
    let call = deep.relations.iter().find(|r| r.kind == "call").expect("the expanded call");
    assert!(call.evidence.as_deref().is_some_and(|e| e.contains("IOrderService")), "{call:?}");
    assert!(deep.elements.len() > d.elements.len());

    // A selection inside a method draws that method and marks the selected statements.
    let file = "backend-dotnet/Services/OrderService.cs";
    let src = std::fs::read_to_string(fixtures().join("knowledge/fullstack").join(file)).unwrap();
    let at = src.find("throw new OrderNotFoundException").unwrap() as u32;
    let sel = diagram("knowledge/fullstack", UmlKind::Activity, UmlScope::Range { file: file.into(), start_byte: at, end_byte: at + 5 }, UmlOptions::default());
    assert_eq!(sel.title, "Activity of OrderService.CancelOrderAsync");
    assert!(sel.mermaid.contains("classDef hl"), "{}", sel.mermaid);
}

#[test]
fn activity_diagrams_in_typescript_and_rust() {
    let ts = diagram("ts/Sample", UmlKind::Activity, UmlScope::Symbol { id: "src/services/orderService.OrderService.describeAll".into() }, UmlOptions::default());
    save("ts_activity_describe_all", &ts);
    assert!(ts.mermaid.contains("-->|\"OrderStatus.Paid\"|") && ts.mermaid.contains("-->|\"default\"|"), "{}", ts.mermaid);
    assert!(ts.mermaid.contains("⇢ produce"), "yield");

    let rs = diagram("rust/Sample", UmlKind::Activity, UmlScope::Symbol { id: "crate::main".into() }, UmlOptions::default());
    save("rust_activity_main", &rs);
    assert!(rs.mermaid.contains("[/\""), "a spawned thread: {}", rs.mermaid);
    assert_eq!(rs.title, "Activity of main.main");

    // One action per statement instead of merged runs.
    let each = diagram("rust/Sample", UmlKind::Activity, UmlScope::Symbol { id: "crate::main".into() }, UmlOptions { granularity: nodex_uml::Granularity::Statement, ..UmlOptions::default() });
    assert!(count(&each, "action") > count(&rs, "action"));
    let code = diagram("rust/Sample", UmlKind::Activity, UmlScope::Symbol { id: "crate::main".into() }, UmlOptions { code_labels: true, ..UmlOptions::default() });
    assert!(code.mermaid.contains("let total"), "{}", code.mermaid);
}

#[test]
fn sequence_follows_a_ui_action_through_http_to_the_database() {
    let opts = UmlOptions { focus: Some("frontend-react/src/components/OrderList.OrderList.onCancel".into()), ..UmlOptions::default() };
    let d = diagram("knowledge/fullstack", UmlKind::Sequence, UmlScope::Folder { path: "frontend-react".into() }, opts);
    save("fullstack_sequence_ui", &d);
    balanced(&d);
    let names: Vec<&str> = d.elements.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(names, vec!["User", "OrderList", "orders", "OrdersController", "OrderService", "Database (EF Core)"]);
    // Front end to back end over HTTP, grouped by component.
    let http = d.relations.iter().find(|r| r.kind == "http").expect("the HTTP request");
    assert_eq!(http.label.as_deref(), Some("POST /api/Orders/{id}/cancel"));
    assert!(http.evidence.as_deref().is_some_and(|e| e.contains("matches")));
    assert!(d.mermaid.contains("box ") && d.mermaid.contains("frontend-react") && d.mermaid.contains("backend-dotnet"));
    // The controller reaches the service through dependency injection, the service the database.
    assert!(d.mermaid.contains("via IOrderService (dependency injection)"), "{}", d.mermaid);
    assert!(d.relations.iter().any(|r| r.kind == "database" && r.label.as_deref() == Some("FindAsync(id) · Orders")));
    assert!(d.mermaid.contains("critical try") && d.mermaid.contains("option catch OrderNotFoundException"));
    assert!(d.mermaid.contains("--x") && d.mermaid.contains("throw OrderNotFoundException"));
    assert!(d.warnings.iter().any(|w| w.contains("declared types")), "no compiler facts in tests");
    insta::assert_snapshot!("fullstack_sequence_ui", d.mermaid);

    // The route handler alone: an authenticated client calls it.
    let api = diagram("knowledge/fullstack", UmlKind::Sequence, UmlScope::Symbol { id: "Shop.Controllers.OrdersController.Cancel(Guid)".into() }, UmlOptions::default());
    save("fullstack_sequence_route", &api);
    balanced(&api);
    assert!(api.mermaid.contains("actor n0 as Authenticated client"), "{}", api.mermaid);
    assert!(api.mermaid.contains("HTTP response"));
}

#[test]
fn sequence_diagrams_in_csharp_and_rust_samples() {
    let cs = diagram("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions::default());
    save("csharp_sequence_main", &cs);
    balanced(&cs);
    assert!(cs.title.ends_with("Program.Main"));
    assert!(cs.mermaid.contains("actor n0 as Operator"));
    assert!(cs.mermaid.contains("create participant") && cs.mermaid.contains("as OrderService"), "{}", cs.mermaid);
    assert!(cs.mermaid.contains("via IOrderRepository (only implementation)"), "{}", cs.mermaid);
    assert!(cs.mermaid.contains("loop ") && cs.mermaid.contains("throw InvalidOperationException"));
    assert!(!cs.mermaid.contains("WriteLine"), "logging is noise");

    // Depth 0: only the root's own messages.
    let flat = diagram("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { depth: Some(0), ..UmlOptions::default() });
    balanced(&flat);
    assert_eq!(flat.mermaid.matches("->>+").count(), 1, "{}", flat.mermaid);
    // Library calls shown when noise is not hidden.
    let noisy = diagram("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { hide_noise: false, ..UmlOptions::default() });
    balanced(&noisy);
    assert!(noisy.mermaid.contains("WriteLine"));

    let rs = diagram("rust/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions::default());
    save("rust_sequence_main", &rs);
    balanced(&rs);
    assert!(rs.mermaid.contains("participant n1 as main"), "{}", rs.mermaid);
    assert!(rs.mermaid.contains("sleep_ms(10)"));
    assert!(rs.mermaid.contains("opt order.status != OrderStatus::Paid"));
}

fn states(focus: &str) -> UmlDiagram {
    let d = diagram("knowledge/uml-states", UmlKind::State, UmlScope::Project, UmlOptions { focus: Some(focus.into()), ..UmlOptions::default() });
    assert!(d.mermaid.starts_with("stateDiagram-v2"), "{focus}: {:?}", d.warnings);
    d
}

/// `from --> to : label` between two named states.
fn transition(d: &UmlDiagram, from: &str, to: &str) -> Option<String> {
    let (a, b) = (id_of(d, from), id_of(d, to));
    d.mermaid.lines().map(str::trim).find_map(|l| {
        let rest = l.strip_prefix(&format!("{a} --> {b}"))?;
        Some(rest.trim_start_matches(" : ").trim().to_owned())
    })
}

#[test]
fn state_diagrams_from_enum_fields_with_guards_switches_and_helpers() {
    let d = states("state:field:States.Order:Status");
    save("states_enum_order", &d);
    assert!(d.mermaid.contains("[*] --> n0"), "a property initializer is the start: {}", d.mermaid);
    // `if (Status != Draft) throw` before the assignment: only from Draft.
    assert_eq!(transition(&d, "Draft", "Submitted").as_deref(), Some("Submit"));
    assert!(transition(&d, "Paid", "Submitted").is_none());
    // A helper guard (`EnsureStatus(Submitted)`) followed into its body; the rest is a guard.
    assert_eq!(transition(&d, "Submitted", "Paid").as_deref(), Some("Pay [amount >= Total]"));
    // `switch (Status) { case Paid: ... }`.
    assert_eq!(transition(&d, "Paid", "Shipped").as_deref(), Some("Ship"));
    // `if (Status == Shipped || Status == Cancelled) return;` then cancel from the others.
    for from in ["Draft", "Submitted", "Paid"] {
        assert_eq!(transition(&d, from, "Cancelled").as_deref(), Some("Cancel"), "{from}");
    }
    assert!(transition(&d, "Shipped", "Cancelled").is_none());
    assert!(transition(&d, "Archived", "Cancelled").is_none(), "a state never set is never left");
    assert!(d.mermaid.contains(&format!("{} --> [*]", id_of(&d, "Cancelled"))), "Cancelled ends the life cycle");
    assert!(d.warnings.iter().any(|w| w.contains("Never set by the code: Archived")));
    assert!(d.elements.iter().find(|e| e.label == "Paid").and_then(|e| e.symbol.as_deref()) == Some("States.OrderStatus.Paid"));
    insta::assert_snapshot!("states_enum_order", d.mermaid);

    let rs = states("state:field:rust::Player:state");
    save("states_enum_rust", &rs);
    assert!(rs.mermaid.contains("[*] --> n0"), "`Player {{ state: PlayerState::Idle }}` in new()");
    assert_eq!(transition(&rs, "Idle", "Jumping").as_deref(), Some("jump"), "match arm `Idle | Running`");
    assert_eq!(transition(&rs, "Running", "Jumping").as_deref(), Some("jump"));
    assert!(transition(&rs, "Dead", "Jumping").is_none(), "`_ => {{}}` changes nothing");
    assert_eq!(transition(&rs, "Idle", "Dead").as_deref(), Some("hit [self.lives == 0]"));
}

#[test]
fn state_diagrams_from_other_detectors() {
    let react = states("state:hook:ts/src/useUpload.useUpload:status");
    save("states_react", &react);
    assert_eq!(transition(&react, "idle", "uploading").as_deref(), Some("upload"));
    assert_eq!(transition(&react, "done", "uploading").as_deref(), Some("upload"), "every value but uploading");
    assert!(transition(&react, "uploading", "uploading").is_none());
    assert_eq!(transition(&react, "uploading", "error").as_deref(), Some("upload"), "the catch clause");
    assert_eq!(transition(&react, "done", "idle").as_deref(), Some("reset"));

    let pattern = states("state:pattern:States.TrafficLight:_state");
    save("states_pattern", &pattern);
    assert_eq!(transition(&pattern, "Red", "Green").as_deref(), Some("Tick (Next)"));
    assert_eq!(transition(&pattern, "Yellow", "Red").as_deref(), Some("Tick (Next)"));
    assert!(pattern.mermaid.contains("[*] --> n0") && pattern.elements[0].label == "Red", "`_state = new Red()`");

    let flags = states("state:flags:States.Connection");
    save("states_flags", &flags);
    assert_eq!(transition(&flags, "none set", "connected").as_deref(), Some("Open"));
    assert_eq!(transition(&flags, "connected", "disposed").as_deref(), Some("Dispose"));
    assert!(transition(&flags, "disposed", "connected").is_none(), "`if (_disposed) throw`");
    assert!(flags.mermaid.contains(&format!("{} --> [*]", id_of(&flags, "disposed"))));

    let lexer = states("state:loop:States.Lexer.CountTokens(string):mode");
    save("states_loop", &lexer);
    assert_eq!(transition(&lexer, "Start", "Number").as_deref(), Some("[char.IsDigit(c)]"));
    assert_eq!(transition(&lexer, "Word", "Start").as_deref(), Some("[!char.IsLetterOrDigit(c)]"));

    let door = states("state:typestate:rust::Door");
    save("states_typestate", &door);
    assert_eq!(transition(&door, "Closed", "Open").as_deref(), Some("open"));
    assert_eq!(transition(&door, "Open", "Closed").as_deref(), Some("close"));

    let xstate = states("state:declared:ts/src/fetchMachine.fetchMachine:fetch");
    save("states_xstate", &xstate);
    assert!(xstate.mermaid.contains(&format!("state {} {{", id_of(&xstate, "success"))), "nested states: {}", xstate.mermaid);
    assert_eq!(transition(&xstate, "failure", "loading").as_deref(), Some("RETRY [canRetry]"));
    assert!(xstate.warnings.iter().all(|w| !w.contains("Read from the code")), "declared machines are exact");

    let stateless = states("state:declared:States.Door:machine");
    save("states_stateless", &stateless);
    assert_eq!(transition(&stateless, "Closed", "Locked").as_deref(), Some("Lock [HasKey]"));

    // A stored entity: changed by a service, triggered by the route that reaches it.
    let entity = diagram("knowledge/fullstack", UmlKind::State, UmlScope::Project, UmlOptions::default());
    save("states_entity", &entity);
    assert!(entity.title.contains("stored records"), "{}", entity.title);
    assert_eq!(transition(&entity, "Pending", "Cancelled").as_deref(), Some("POST /api/Orders/‹id›/cancel [order != null]"));
    assert!(entity.candidates.len() >= 2);
}

#[test]
fn use_case_diagrams_find_actors_includes_and_extensions() {
    let d = diagram("knowledge/auth", UmlKind::UseCase, UmlScope::Folder { path: "dotnet".into() }, UmlOptions::default());
    save("usecase_auth", &d);
    let actors: Vec<&str> = d.elements.iter().filter(|e| e.kind == "actor").map(|e| e.label.as_str()).collect();
    for a in ["Admin", "Ops", "Visitor", "Authenticated user"] {
        assert!(actors.contains(&a), "{a} in {actors:?}");
    }
    let cases: Vec<&str> = d.elements.iter().filter(|e| e.kind == "use case").map(|e| e.label.as_str()).collect();
    for c in ["View reports", "Purge", "View health", "Restart", "View status"] {
        assert!(cases.contains(&c), "{c} in {cases:?}");
    }
    // Roles are kinds of authenticated users; anonymous routes belong to visitors.
    assert!(d.relations.iter().any(|r| r.kind == "generalization" && r.from == id_of(&d, "Admin")));
    assert!(d.relations.iter().any(|r| r.kind == "association" && r.from == id_of(&d, "Visitor") && r.to == id_of(&d, "View health")));
    // An operation two use cases share: always called by one («include»), only under a
    // condition by the other («extend»).
    assert!(relation(&d, "View reports", "Record access").is_some_and(|r| r.kind == "include"));
    assert!(relation(&d, "Record access", "Purge").is_some_and(|r| r.kind == "extend"));
    insta::assert_snapshot!("usecase_auth", d.mermaid);

    let full = diagram("knowledge/fullstack", UmlKind::UseCase, UmlScope::Project, UmlOptions::default());
    save("usecase_fullstack", &full);
    // The UI action sends the request a route serves: it includes that use case.
    let ui = relation(&full, "Refresh", "Get orders").expect("UI → API");
    assert_eq!(ui.kind, "include");
    assert!(full.mermaid.contains("✎ Order"), "effects of a use case");
    assert!(!full.elements.iter().any(|e| e.label.contains("Top-level")), "a web host's main is not a use case");
}

fn span(id: u32, flow: &str, parent: Option<u32>, start: i64, end: i64) -> CallSpan {
    CallSpan { id, flow: flow.into(), parent, start, end, depth: parent.map(|_| 1).unwrap_or(0), ..CallSpan::default() }
}

#[test]
fn recorded_sequences_follow_calls_and_compress_loops() {
    let main = "Sample.Program.Main(string[])";
    let add = "Sample.Services.InMemoryOrderRepository.Add(Order)";
    let ship = "Sample.Services.OrderService.ShipAsync(int)";
    let find = "Sample.Services.InMemoryOrderRepository.Find(int)";
    let mut spans = vec![span(1, main, None, 0, 9_000)];
    for i in 0..3 {
        spans.push(span(2 + i, add, Some(1), 100 + i as i64 * 10, 105 + i as i64 * 10));
    }
    let mut s = span(10, ship, Some(1), 1_000, 8_000);
    s.args = vec!["id = 2".into()];
    s.threw = true;
    s.exception = Some("InvalidOperationException: not paid".into());
    spans.push(s);
    let mut f = span(11, find, Some(10), 1_100, 1_200);
    f.ret = Some("Order #2".into());
    spans.push(f);
    let rt = Runtime { spans, ..Runtime::default() };
    let d = diagram_with("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { runtime: true, ..UmlOptions::default() }, Some(&rt));
    save("recorded_sequence", &d);
    balanced(&d);
    assert_eq!(d.title, "Recorded sequence of Program.Main #1");
    assert!(d.mermaid.contains("loop 3×"), "three identical Add calls: {}", d.mermaid);
    assert_eq!(d.mermaid.matches("Add(").count(), 1);
    assert!(d.mermaid.contains("ShipAsync(id = 2)") && d.mermaid.contains("Order #35;2"), "the return value: {}", d.mermaid);
    assert!(d.mermaid.contains("--x-") && d.mermaid.contains("InvalidOperationException"));
    assert_eq!(d.candidates.first().map(|c| c.id.as_str()), Some("span:1"));
    // Another recorded call as the root.
    let focused = diagram_with("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { runtime: true, focus: Some("span:10".into()), ..UmlOptions::default() }, Some(&rt));
    balanced(&focused);
    assert!(focused.title.ends_with("ShipAsync #10"), "{}", focused.title);
    // As .NET records them: iterator steps and async methods are resumed executions (even the
    // first), and an async method keeps its exception in its task instead of throwing.
    let describe = "Sample.Services.OrderService.DescribeAll()";
    let mut spans = vec![span(1, main, None, 0, 9_000)];
    spans[0].resumed = true;
    for i in 0..3u32 {
        let mut step = span(20 + i * 2, describe, Some(1), 100 + i as i64 * 100, 150 + i as i64 * 100);
        step.resumed = true;
        spans.push(step);
        spans.push(span(21 + i * 2, find, Some(20 + i * 2), 110 + i as i64 * 100, 120 + i as i64 * 100));
    }
    let mut ship = span(40, ship, Some(1), 1_000, 2_000);
    ship.resumed = true;
    ship.exception = Some("InvalidOperationException: not paid".into());
    spans.push(ship);
    let rt = Runtime { spans, ..Runtime::default() };
    let net = diagram_with("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { runtime: true, ..UmlOptions::default() }, Some(&rt));
    save("recorded_sequence_async", &net);
    balanced(&net);
    assert!(net.mermaid.contains("loop 3×") && net.mermaid.contains("DescribeAll() ⟳"), "{}", net.mermaid);
    assert!(net.mermaid.contains("as OrderService") && net.mermaid.contains("Note over n2: ⚠ InvalidOperationException"), "{}", net.mermaid);

    // Nothing recorded: a hint, no diagram.
    let none = diagram("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions { runtime: true, ..UmlOptions::default() });
    assert!(none.mermaid.is_empty() && none.warnings[0].contains("No recorded calls"));
}

#[test]
fn recorded_runs_mark_what_ran_in_activity_and_static_sequence_diagrams() {
    let mut project = open("csharp/Sample");
    let id = "Sample.Services.OrderService.ShipAsync(int)";
    let flow = project.flow(id).unwrap();
    let first = flow.nodes.iter().find(|n| n.concept == nodex_ir::Concept::Branch).unwrap().id.clone();
    let mut rt = Runtime::default();
    rt.counts.entry(id.into()).or_default().insert(first, 4);
    rt.calls.insert(id.into(), 4);
    let d = diagram_with("csharp/Sample", UmlKind::Activity, UmlScope::Symbol { id: id.into() }, UmlOptions::default(), Some(&rt));
    save("activity_overlay", &d);
    assert!(d.mermaid.contains("classDef hot") && d.mermaid.contains("classDef cold"), "{}", d.mermaid);
    assert!(d.elements.iter().any(|e| e.detail.as_deref().is_some_and(|x| x.contains("ran 4×"))));
    let seq = diagram_with("csharp/Sample", UmlKind::Sequence, UmlScope::Project, UmlOptions::default(), Some(&rt));
    assert!(seq.mermaid.contains("ShipAsync(2) ▶4"), "{}", seq.mermaid);
}

#[test]
fn availability_offers_states_and_use_cases() {
    let mut project = open("knowledge/uml-states");
    let graph = project.structure();
    let knowledge = project.knowledge();
    let units = project.units();
    let flows = std::sync::Mutex::new(project);
    let input = UmlInput { graph: &graph, knowledge: &knowledge, units: &units, project: "states", flows: &flows, runtime: None };
    let order = availability(&input, &UmlScope::Symbol { id: "States.Order".into() }).unwrap();
    assert_eq!(order.suggested, Some(UmlKind::State), "{order:?}");
    assert!(order.kinds.iter().any(|k| k.kind == UmlKind::State && k.available && k.reason.contains("Order.Status")));
    let lexer = availability(&input, &UmlScope::Symbol { id: "States.Lexer.CountTokens(string)".into() }).unwrap();
    assert!(lexer.kinds.iter().any(|k| k.kind == UmlKind::State && k.available), "{lexer:?}");
    assert!(lexer.kinds.iter().any(|k| k.kind == UmlKind::UseCase && !k.available));
}
