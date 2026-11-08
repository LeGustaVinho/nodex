//! Semantic enrichment: synthetic facts (always run) and the real Roslyn sidecar (`--ignored`).

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::Project;
use nodex_ir::semantic::{ByteRange, NavigationFact, RefKind, SemanticFacts, SemanticRef, SemanticState, SemanticTarget};
use nodex_ir::{Concept, Expr, StructEdgeKind};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::CSharpFrontend;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(CSharpFrontend)];
    Project::open(&fixtures().join(rel), frontends).expect("fixture opens")
}

fn find_expr<'a>(e: &'a Expr, pred: &dyn Fn(&Expr) -> bool) -> Option<&'a Expr> {
    if pred(e) {
        return Some(e);
    }
    e.children.iter().find_map(|c| find_expr(c, pred))
}

const MAIN: &str = "Sample.Program.Main(string[])";
const TOTAL: &str = "Sample.Services.OrderService.TotalForCustomer(string)";

/// Facts built from the IR's own spans, as a provider would report them.
#[test]
fn synthetic_call_fact_links_flow_and_structure() {
    let mut project = open("csharp/Sample");
    let main = project.flow(MAIN).unwrap();
    let structure = project.structure();
    let total = structure.symbols.iter().find(|s| s.id == TOTAL).unwrap();
    let decl = &total.origins[0];
    let call_node = main.nodes.iter().find(|n| n.label.contains("TotalForCustomer")).unwrap();
    let call_expr = find_expr(call_node.expr.as_ref().unwrap(), &|e| e.concept == Concept::Call).unwrap();

    let facts = SemanticFacts {
        source: "test".into(),
        refs: vec![SemanticRef {
            site: ByteRange { file: "Program.cs".into(), start: call_expr.span.start_byte, end: call_expr.span.end_byte },
            kind: RefKind::Call,
            target: SemanticTarget {
                decl: Some(ByteRange { file: decl.file.clone(), start: decl.start_byte, end: decl.end_byte }),
                display: "OrderService.TotalForCustomer(string)".into(),
                container: Some("Sample.Services.OrderService".into()),
                kind: "method".into(),
                linq: false,
                external: false,
            },
        }],
        locals: Vec::new(),
        navigation: None,
        implementation_contracts: Vec::new(),
        configuration_files: Vec::new(),
        messages: Vec::new(),
    };
    project.apply_semantic(nodex_ir::Language::Csharp, facts);

    let structure = project.structure();
    assert!(structure.edges.iter().any(|e| e.from == MAIN && e.to == TOTAL && e.kind == StructEdgeKind::Calls));
    let main = project.flow(MAIN).unwrap();
    let node = main.nodes.iter().find(|n| n.id == call_node.id).unwrap();
    assert_eq!(node.calls[0].symbol.as_deref(), Some(TOTAL));
    let annotated = find_expr(node.expr.as_ref().unwrap(), &|e| e.symbol.is_some()).expect("expression annotated");
    assert_eq!(annotated.target.as_deref(), Some("OrderService.TotalForCustomer(string)"));
    assert!(project.summary().semantic.iter().any(|s| s.state == SemanticState::Ready));
}

#[test]
fn navigation_uses_compiler_sites_and_hides_stale_results_while_loading() {
    use nodex_core::navigation::NavigationKind;
    let mut project = open("csharp/Sample");
    let graph = project.structure();
    let decl = &graph.symbols.iter().find(|s| s.id == TOTAL).unwrap().origins[0];
    let source = project.source("Program.cs").unwrap();
    let start = source.find("TotalForCustomer(").unwrap() as u32;
    let site = ByteRange { file: "Program.cs".into(), start, end: start + "TotalForCustomer".len() as u32 };
    let fact = NavigationFact {
        site: site.clone(),
        target: SemanticTarget {
            decl: Some(ByteRange { file: decl.file.clone(), start: decl.start_byte, end: decl.end_byte }),
            display: TOTAL.into(), container: None, kind: "method".into(), linq: false, external: false,
        },
        implementation: false,
        implementation_kind: None,
    };
    project.apply_semantic(nodex_ir::Language::Csharp, SemanticFacts {
        navigation: Some(vec![fact.clone(), fact]),
        ..Default::default()
    });
    project.structure();
    let usages = |p: &Project| serde_json::to_value(p.navigation(TOTAL, NavigationKind::Usages).unwrap().resolve()).unwrap();
    let result = usages(&project);
    assert_eq!(result["coverage"], "indexed");
    let locations = result["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0]["span"]["start_byte"], start);
    assert_eq!(locations[0]["symbol"], MAIN);
    assert!(locations[0]["snippet"].as_str().unwrap().contains("TotalForCustomer"));
    project.set_semantic_status(nodex_ir::Language::Csharp, SemanticState::Loading, "updating");
    let result = usages(&project);
    assert_eq!(result["coverage"], "loading");
    assert_eq!(result["locations"].as_array().unwrap().len(), 0);
}

#[test]
fn member_expression_navigation_requires_an_unambiguous_reference() {
    let mut project = open("csharp/Sample");
    let graph = project.structure();
    let file = "Services/OrderService.cs";
    let source = project.source(file).unwrap();
    let start = source.find("order.Amount").unwrap() as u32 + "order.".len() as u32;
    let site = ByteRange { file: file.into(), start, end: start + "Amount".len() as u32 };
    let fact_for = |name: &str| {
        let member = graph.symbols.iter().find(|s| s.name == name && s.kind == nodex_ir::SymbolKind::Property).unwrap();
        let decl = &member.origins[0];
        NavigationFact {
            site: site.clone(),
            target: SemanticTarget {
                decl: Some(ByteRange { file: decl.file.clone(), start: decl.start_byte, end: decl.end_byte }),
                display: member.name.clone(), container: None, kind: "property".into(), linq: false, external: false,
            },
            implementation: false,
            implementation_kind: None,
        }
    };
    let resolve = |project: &mut Project, navigation| {
        project.apply_semantic(nodex_ir::Language::Csharp, SemanticFacts {
            navigation: Some(navigation),
            ..Default::default()
        });
        let flow = project.flow(TOTAL).unwrap();
        let member = flow.nodes.iter().filter_map(|node| node.expr.as_ref()).find_map(|expr| {
            find_expr(expr, &|e| e.concept == Concept::GetMember && e.label == ".Amount")
        }).unwrap();
        member.symbol.clone()
    };
    assert!(resolve(&mut project, vec![fact_for("Amount")]).is_some_and(|id| id.ends_with(".Amount")));
    assert_eq!(resolve(&mut project, vec![fact_for("Amount"), fact_for("Status")]), None);
}

#[test]
fn msbuild_project_changes_invalidate_csharp_analysis() {
    let mut project = open("csharp/Sample");
    project.apply_semantic(nodex_ir::Language::Csharp, SemanticFacts::default());
    let config = project.root().join("Sample.csproj");
    assert!(config.is_file());
    let revision = project.source_revision();

    assert_eq!(project.refresh_path(&config).as_deref(), Some("Sample.csproj"));
    assert_eq!(project.source_revision(), revision + 1);
    assert_eq!(project.summary().semantic[0].state, SemanticState::Loading);
    assert!(project.refresh_path(&config).is_none());
}

/// End-to-end with the real sidecar. Build it first:
/// `dotnet build sidecars/dotnet/Nodex.Roslyn -c Release`, then `cargo test -- --ignored`.
#[test]
#[ignore = "needs the .NET SDK and a built Nodex.Roslyn sidecar"]
fn roslyn_sidecar_resolves_the_sample() {
    let mut project = open("csharp/Sample");
    let mut provider = CSharpFrontend
        .semantic(project.root())
        .expect("C# has a provider")
        .expect("sidecar starts");
    let facts = provider.analyze().expect("analysis succeeds");
    assert!(facts.source.contains("MSBuild"), "{}", facts.source);
    project.apply_semantic(nodex_ir::Language::Csharp, facts);

    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    assert!(has(MAIN, TOTAL, StructEdgeKind::Calls));
    assert!(has(
        "Sample.Services.OrderService.ShipAsync(int)",
        "Sample.Services.OrderService.OrderShipped",
        StructEdgeKind::Raises
    ));
    assert!(has("Sample.Services.OrderService.OrderShipped", MAIN, StructEdgeKind::Subscribes));
    assert!(has("Sample.Services.AuditScope", "ext:System.IDisposable", StructEdgeKind::Implements));
    assert!(!graph.symbols.iter().any(|s| s.id == "ext:IDisposable"), "name-based guess replaced");

    // `var` locals get their inferred types; the LINQ pipeline is confirmed.
    let main = project.flow(MAIN).unwrap();
    let repo = main.nodes.iter().find(|n| n.label.starts_with("repository =")).unwrap();
    let pin = repo.outputs.iter().find(|p| p.id == "var:repository").unwrap();
    assert_eq!(pin.ty.as_deref(), Some("InMemoryOrderRepository"));
    let linq = project.flow("Sample.Services.OrderService.TotalForCustomerLinq(string)").unwrap();
    let ret = linq.nodes.iter().find(|n| n.concept == Concept::Return).unwrap();
    let stage = find_expr(ret.expr.as_ref().unwrap(), &|e| e.label.starts_with("Where")).unwrap();
    assert!(stage.target.as_deref().is_some_and(|t| t.contains("Enumerable.Where")), "{:?}", stage.target);
    assert!(!stage.flavors.iter().any(|f| f == "custom-stage"));

    // Edits are picked up through `update`.
    let file = project.root().join("Program.cs");
    let facts = provider.update(&[file]).expect("update succeeds");
    assert!(!facts.refs.is_empty());
}

#[test]
#[ignore = "needs the .NET SDK and a built Nodex.Roslyn sidecar"]
fn roslyn_sidecar_handles_unity_without_the_engine() {
    let mut project = open("unity");
    let mut provider = CSharpFrontend.semantic(project.root()).unwrap().expect("sidecar starts");
    let facts = provider.analyze().expect("analysis succeeds");
    assert!(facts.source.contains("loose files"), "{}", facts.source);
    project.apply_semantic(nodex_ir::Language::Csharp, facts);
    let graph = project.structure();
    // UnityEngine is missing, but project-internal calls still resolve.
    assert!(graph.edges.iter().any(|e| e.from == "Game.PlayerController.OnCollisionEnter(Collision)"
        && e.to == "Game.PlayerController.Flash()"
        && e.kind == StructEdgeKind::Calls));
    assert!(graph.edges.iter().any(|e| e.from == "Game.PlayerController" && e.to == "ext:MonoBehaviour"));
}

/// `pnpm --dir sidecars/node/nodex-ts install`, then `cargo test -- --ignored`.
#[test]
#[ignore = "needs Node.js and an installed nodex-ts sidecar"]
fn typescript_sidecar_resolves_the_sample() {
    use nodex_lang_typescript::TypeScriptFrontend;
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(TypeScriptFrontend)];
    let mut project = Project::open(&fixtures().join("ts/Sample"), frontends).expect("fixture opens");
    let mut provider = TypeScriptFrontend.semantic(project.root()).expect("TypeScript has a provider").expect("sidecar starts");
    let facts = provider.analyze().expect("analysis succeeds");
    assert!(facts.source.contains("tsconfig.json"), "{}", facts.source);
    project.apply_semantic(nodex_ir::Language::Typescript, facts);

    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    // Calls resolve through imports, barrels and arrow functions bound to constants.
    assert!(has("src/main.main", "src/services/orderService.OrderService.totalForCustomer", StructEdgeKind::Calls));
    assert!(has("src/services/orderService.OrderService.ship", "src/util/timing.sleep", StructEdgeKind::Calls));
    assert!(has("src/services/orderService.OrderService.ship", "src/services/orderService.fail", StructEdgeKind::Calls));
    assert!(has("src/services/orderService.OrderService.ship", "src/domain/order.Order.withStatus", StructEdgeKind::Calls));
    assert!(has("src/main.main", "src/domain/order.Order", StructEdgeKind::Instantiates));
    assert!(has("src/domain/shapes.Circle", "src/domain/shapes.Shape", StructEdgeKind::Inherits));
    assert!(has("src/domain/shapes.Shape", "src/domain/order.Entity", StructEdgeKind::Implements));
    // Structural typing: Order has every member of Priced without declaring it.
    assert!(has("src/domain/order.Order", "src/domain/order.Priced", StructEdgeKind::Compatible));
    assert!(!has("src/domain/order.Order", "src/domain/order.Entity", StructEdgeKind::Compatible), "declared contracts are not repeated");

    // Inferred types reach the pins; array stages are confirmed as standard operations.
    let main = project.flow("src/main.main").unwrap();
    let repo = main.nodes.iter().find(|n| n.label.starts_with("repository =")).unwrap();
    let pin = repo.outputs.iter().find(|p| p.id == "var:repository").unwrap();
    assert_eq!(pin.ty.as_deref(), Some("InMemoryOrderRepository"));
    let call = main.nodes.iter().find(|n| n.label.contains("totalForCustomer")).unwrap();
    assert!(call.calls.iter().any(|c| c.symbol.as_deref() == Some("src/services/orderService.OrderService.totalForCustomer")));
    let pipeline = project.flow("src/services/orderService.OrderService.totalForCustomerPipeline").unwrap();
    let ret = pipeline.nodes.iter().find(|n| n.concept == Concept::Return).unwrap();
    let stage = find_expr(ret.expr.as_ref().unwrap(), &|e| e.label.starts_with("filter")).unwrap();
    assert!(!stage.flavors.iter().any(|f| f == "custom-stage"), "{:?}", stage.flavors);
    assert!(stage.target.as_deref().is_some_and(|t| t.starts_with("Array.filter")), "{:?}", stage.target);
}

/// `rustup component add rust-analyzer`, then `cargo test -- --ignored`.
#[test]
#[ignore = "needs rust-analyzer"]
fn rust_analyzer_resolves_the_sample() {
    use nodex_lang_rust::RustFrontend;
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(RustFrontend)];
    let mut project = Project::open(&fixtures().join("rust/Sample"), frontends).expect("fixture opens");
    let mut provider = RustFrontend.semantic(project.root()).expect("Rust has a provider").expect("rust-analyzer starts");
    let facts = provider.analyze().expect("analysis succeeds");
    assert!(facts.source.contains("rust-analyzer"), "{}", facts.source);
    project.apply_semantic(nodex_ir::Language::Rust, facts);

    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    // Calls resolve across modules, through `use` and generic receivers.
    assert!(has("crate::main", "crate::services::order_service::OrderService::total_for_customer", StructEdgeKind::Calls));
    assert!(has("crate::main", "crate::ship_all", StructEdgeKind::Calls));
    assert!(has("crate::services::order_service::OrderService::ship", "crate::util::sleep_ms", StructEdgeKind::Calls));
    assert!(has("crate::services::order_service::OrderService::ship", "crate::domain::order::Order::with_status", StructEdgeKind::Calls));
    assert!(has("crate::main", "crate::domain::order::Order", StructEdgeKind::Instantiates));

    // Types reach the pins; std methods are external targets that operations recognize.
    let total = project.flow("crate::services::order_service::OrderService::total_for_customer").unwrap();
    let order = total.nodes.iter().find(|n| n.concept == Concept::LoopForEach).unwrap();
    assert_eq!(order.outputs.iter().find(|p| p.id == "var:order").and_then(|p| p.ty.as_deref()), Some("Order"));
    let main = project.flow("crate::main").unwrap();
    let call = main.nodes.iter().find(|n| n.label.contains("total_for_customer")).unwrap();
    assert!(call.calls.iter().any(|c| c.symbol.as_deref() == Some("crate::services::order_service::OrderService::total_for_customer")));
    let repo = project.flow("crate::services::repository::InMemoryOrderRepository::add").unwrap();
    let push = repo.nodes.iter().find(|n| n.label.contains("push")).unwrap();
    let push_expr = find_expr(push.expr.as_ref().unwrap(), &|e| e.label == "push").unwrap();
    assert!(push_expr.target.as_deref().is_some_and(|t| t.starts_with("alloc::vec::Vec::push")), "{:?}", push_expr.target);
    assert_eq!(push_expr.op.as_deref(), Some("list.add"));
}
