use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nodex_core::Project;
use nodex_ir::knowledge::Confidence;
use nodex_uml::architecture::{
    self, ArchitectureConfig, ArchitectureDiagram, ArchitectureKind, ArchitectureRequest,
    ArchitectureView, DeclaredApplication, DeclaredElement, DeclaredRelation, Layer,
    LibraryMembership, Provenance,
};
use nodex_uml::{UmlInput, UmlScope};

fn diagram(
    project: &str,
    request: ArchitectureRequest,
    config: &ArchitectureConfig,
) -> ArchitectureDiagram {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(project);
    diagram_at(&folder, request, config)
}

fn diagram_at(
    folder: &Path,
    request: ArchitectureRequest,
    config: &ArchitectureConfig,
) -> ArchitectureDiagram {
    let mut project = Project::open(
        folder,
        vec![
            Arc::new(nodex_lang_csharp::CSharpFrontend),
            Arc::new(nodex_lang_typescript::TypeScriptFrontend),
            Arc::new(nodex_lang_rust::RustFrontend),
        ],
    )
    .unwrap();
    let graph = project.structure();
    let knowledge = project.knowledge();
    let units = project.units();
    let flows = Mutex::new(project);
    let input = UmlInput {
        graph: &graph,
        knowledge: &knowledge,
        units: &units,
        project: "Sample",
        flows: &flows,
        runtime: None,
    };
    architecture::build(&input, &request, config).unwrap()
}

fn request(view: ArchitectureView) -> ArchitectureRequest {
    ArchitectureRequest {
        view,
        scope: UmlScope::Project,
        expanded: Default::default(),
        container: None,
    }
}

fn project_with_files(files: &[(&str, &str)]) -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    for (path, source) in files {
        let file = project.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, source).unwrap();
    }
    project
}

#[test]
fn a_c4_declaration_does_not_change_layer_module_keys() {
    let project = project_with_files(&[
        ("package.json", r#"{"name":"shop"}"#),
        ("src/services/orders.ts", "export function orders() {}"),
    ]);
    let mut config = ArchitectureConfig::default();
    config.layers.insert("src/services".into(), Layer::Domain);
    config.applications.push(DeclaredApplication {
        root: "src/services".into(),
        name: "Orders".into(),
        technology: "Node.js".into(),
    });
    let d = diagram_at(project.path(), request(ArchitectureView::Layers), &config);
    assert!(d.modules.iter().any(|n| n.id == "module:src/services"
        && n.layer == Some(Layer::Domain)
        && n.provenance == Provenance::Declared));
}

#[test]
fn layers_project_namespace_imports_onto_visible_modules() {
    let project = project_with_files(&[
        (
            "Controllers/Orders.cs",
            "using Shop.Domain; namespace Shop.Controllers; public class Orders {}",
        ),
        (
            "Domain/Order.cs",
            "namespace Shop.Domain; public class Order {}",
        ),
        (
            "Domain/Warehouse.cs",
            "namespace Shop.Domain; public class Warehouse {}",
        ),
    ]);
    let mut req = request(ArchitectureView::Layers);
    req.expanded.extend([Layer::Interface, Layer::Domain]);
    let d = diagram_at(project.path(), req, &ArchitectureConfig::default());
    assert!(
        d.edges.iter().any(|e| e.kind == "Imports"
            && e.from == "module:Controllers"
            && e.to == "module:Domain"),
        "{:?}",
        d.edges
    );
}

#[test]
fn layers_use_actual_references_and_keep_an_editable_module_when_collapsed() {
    let mut req = request(ArchitectureView::Layers);
    req.scope = UmlScope::Folder {
        path: "backend-dotnet".into(),
    };
    let d = diagram("knowledge/fullstack", req, &ArchitectureConfig::default());
    for (name, layer) in [
        ("Controllers", Layer::Interface),
        ("Services", Layer::Application),
        ("Data", Layer::Infrastructure),
    ] {
        assert!(
            d.modules
                .iter()
                .any(|n| n.name == name && n.layer == Some(layer)),
            "{name} missing from {:?}",
            d.modules
        );
    }
    assert!(
        d.modules
            .iter()
            .any(|n| n.layer == Some(Layer::Unclassified))
    );
    let usage = d.edges.iter().find(|e| {
        e.from == "layer:Application"
            && e.to == "layer:Infrastructure"
            && e.evidence.iter().any(|site| {
                site.path
                    .as_ref()
                    .is_some_and(|p| p.ends_with("OrderService.cs"))
            })
    });
    assert!(usage.is_some(), "{:?}", d.edges);
    let mut config = ArchitectureConfig::default();
    config
        .layers
        .insert("backend-dotnet/Services".into(), Layer::Domain);
    let mut req = request(ArchitectureView::Layers);
    req.scope = UmlScope::Folder {
        path: "backend-dotnet".into(),
    };
    req.expanded.insert(Layer::Domain);
    let d = diagram("knowledge/fullstack", req, &config);
    assert!(d.nodes.iter().any(|n| n.name == "Services"
        && n.parent.as_deref() == Some("layer:Domain")
        && n.provenance == Provenance::Declared));
}

#[test]
fn c4_rolls_up_http_and_places_only_the_selected_application_inside_the_component_view() {
    let mut config = ArchitectureConfig::default();
    config.applications.push(DeclaredApplication {
        root: "frontend-react".into(),
        name: "Shop frontend".into(),
        technology: "React".into(),
    });
    config.actors.push(DeclaredElement {
        id: "buyer".into(),
        name: "Buyer".into(),
        technology: String::new(),
    });
    config.stores.push(DeclaredElement {
        id: "orders".into(),
        name: "Orders".into(),
        technology: "PostgreSQL".into(),
    });
    config.relations.push(DeclaredRelation {
        from: "actor:buyer".into(),
        to: "app:frontend-react".into(),
        label: "Places orders".into(),
    });
    config.relations.push(DeclaredRelation {
        from: "app:backend-dotnet".into(),
        to: "store:orders".into(),
        label: "Reads orders".into(),
    });
    let context = diagram(
        "knowledge/fullstack",
        request(ArchitectureView::Context),
        &config,
    );
    assert!(context.edges.iter().any(|e| e.from == "actor:buyer"
        && e.to == "system"
        && e.provenance == Provenance::Declared));
    assert!(
        context
            .edges
            .iter()
            .any(|e| e.from == "system" && e.to == "host:api.stripe.com")
    );
    let containers = diagram(
        "knowledge/fullstack",
        request(ArchitectureView::Containers),
        &config,
    );
    assert!(
        containers
            .nodes
            .iter()
            .any(|n| n.id == "store:orders" && n.parent.as_deref() == Some("system"))
    );
    assert!(
        containers
            .edges
            .iter()
            .any(|e| e.from == "app:frontend-react"
                && e.to == "app:backend-dotnet"
                && e.kind == "HTTP"
                && e.confidence == Confidence::Inferred)
    );
    let mut req = request(ArchitectureView::Components);
    req.container = Some("app:backend-dotnet".into());
    let components = diagram("knowledge/fullstack", req, &config);
    assert!(
        components
            .nodes
            .iter()
            .filter(|n| n.kind == ArchitectureKind::Component)
            .all(|n| n.parent.as_deref() == Some("app:backend-dotnet"))
    );
    let orders = components
        .nodes
        .iter()
        .find(|n| n.name == "Controllers")
        .unwrap();
    assert!(orders.code.is_some());
    assert!(
        components
            .edges
            .iter()
            .any(|e| e.from == "app:frontend-react" && e.to == orders.id)
    );
    assert!(!components.nodes.iter().any(|n| n.id == "app:backend-nest"));
    assert!(components.mermaid.contains("subgraph"));
}

#[test]
fn a_nest_global_prefix_does_not_establish_runtime_membership() {
    let project = "knowledge/nest-libs";
    let mut req = request(ArchitectureView::Components);
    req.container = Some("app:apps/api".into());
    let d = diagram(project, req, &ArchitectureConfig::default());
    assert!(d.pending.iter().any(|e| {
        e.text.contains("GET /api/orders")
            && e.path
                .as_ref()
                .is_some_and(|p| p.starts_with("libs/orders/"))
    }));
    assert!(
        !d.nodes
            .iter()
            .any(|n| n.path.as_deref() == Some("libs/orders"))
    );
    let mut config = ArchitectureConfig::default();
    config.libraries.push(LibraryMembership {
        application: "app:apps/api".into(),
        path: "libs/orders".into(),
    });
    let mut req = request(ArchitectureView::Components);
    req.container = Some("app:apps/api".into());
    let d = diagram(project, req, &config);
    assert!(
        d.nodes
            .iter()
            .any(|n| n.path.as_deref() == Some("libs/orders"))
    );
    assert!(
        d.pending
            .iter()
            .all(|e| !e.text.contains("GET /api/orders"))
    );
}

#[test]
fn browser_bootstraps_are_recognized() {
    for (fixture, technology) in [("web/react-vite", "React"), ("web/vue-vite", "Vue")] {
        let d = diagram(
            fixture,
            request(ArchitectureView::Containers),
            &ArchitectureConfig::default(),
        );
        assert!(
            d.nodes
                .iter()
                .any(|n| n.kind == ArchitectureKind::Application
                    && n.technology.as_deref() == Some(technology)),
            "{fixture}: {:?}",
            d.nodes
        );
    }
}

#[test]
fn c4_separates_browser_and_server_in_the_same_package() {
    let project = project_with_files(&[
        (
            "package.json",
            r#"{"name":"shop","dependencies":{"express":"*","react-dom":"*"}}"#,
        ),
        (
            "src/frontend/main.tsx",
            r#"
            import { createRoot } from "react-dom/client";
            import { Orders } from "./Orders";
            createRoot(document.getElementById("root")!).render(<Orders />);
        "#,
        ),
        (
            "src/frontend/Orders.tsx",
            r#"
            export function Orders() { fetch("/api/orders"); return <div />; }
        "#,
        ),
        (
            "src/server/index.ts",
            r#"
            import express from "express";
            import { getOrders } from "./orders";
            const app = express();
            app.get("/api/orders", getOrders);
            export function bootstrap() { app.listen(3000); }
            bootstrap();
        "#,
        ),
        (
            "src/server/orders.ts",
            "export function getOrders(req: any, res: any) { res.json([]); }",
        ),
    ]);
    let mut config = ArchitectureConfig::default();
    config.applications.push(DeclaredApplication {
        root: "".into(),
        name: "Shop".into(),
        technology: String::new(),
    });
    config.applications.push(DeclaredApplication {
        root: "src/server".into(),
        name: "Orders API".into(),
        technology: String::new(),
    });
    config.actors.push(DeclaredElement {
        id: "operator".into(),
        name: "Operator".into(),
        technology: String::new(),
    });
    config.relations.push(DeclaredRelation {
        from: "actor:operator".into(),
        to: "app:src/server".into(),
        label: "Checks orders".into(),
    });
    let d = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    assert_eq!(d.applications.len(), 2, "{:?}", d.applications);
    let app = |tech: &str| {
        d.applications
            .iter()
            .find(|n| n.technology.as_deref() == Some(tech))
            .unwrap()
    };
    let (client, server) = (app("React"), app("Node.js"));
    assert_eq!(server.name, "Orders API");
    assert_eq!(server.provenance, Provenance::Declared);
    assert!(
        d.edges
            .iter()
            .any(|e| e.from == client.id && e.to == server.id && e.kind == "HTTP"),
        "{:?}",
        d.edges
    );
    assert!(d.edges.iter().any(|e| e.from == "actor:operator"
        && e.to == server.id
        && e.provenance == Provenance::Declared));
    let mut req = request(ArchitectureView::Components);
    req.container = Some("app:src/server".into());
    let components = diagram_at(project.path(), req, &config);
    assert!(
        components
            .nodes
            .iter()
            .any(|n| n.kind == ArchitectureKind::Component
                && n.path.as_deref() == Some("src/server"))
    );
    assert!(
        components
            .nodes
            .iter()
            .filter(|n| n.kind == ArchitectureKind::Component)
            .all(|n| n.path.as_deref() != Some("src/frontend"))
    );
    assert!(components.nodes.iter().any(|n| n.id == client.id));
    config.excluded_apps.insert("app:src/server".into());
    let scoped = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    assert!(scoped.nodes.iter().any(|n| n.id == client.id));
    assert!(scoped.nodes.iter().all(|n| n.id != server.id));
    config.excluded_apps.insert("app:".into());
    let excluded = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    assert!(
        excluded
            .nodes
            .iter()
            .all(|n| n.kind != ArchitectureKind::Application)
    );
}

#[test]
fn c4_uses_cargo_targets_to_find_rust_executables() {
    let project = project_with_files(&[
        (
            "Cargo.toml",
            r#"
            [package]
            name = "tools"
            version = "0.1.0"
            edition = "2024"
            [[bin]]
            name = "exporter"
            path = "src/cli/export.rs"
        "#,
        ),
        ("src/main.rs", "fn main() {}"),
        ("src/bin/producer.rs", "fn main() {}"),
        ("src/bin/consumer/main.rs", "fn main() {}"),
        ("src/bin/consumer/helper.rs", "pub fn helper() {}"),
        (
            "src/cli/export.rs",
            "fn main() {} mod test_only { fn main() {} }",
        ),
        ("src/lib.rs", "pub fn main() {}"),
        ("src/bin/consumer/nested/helper.rs", "pub fn main() {}"),
        ("examples/demo.rs", "fn main() {}"),
    ]);
    let config = ArchitectureConfig::default();
    let d = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    let names: std::collections::BTreeSet<_> =
        d.applications.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["consumer", "exporter", "producer", "tools"].into());
    let consumer = d
        .applications
        .iter()
        .find(|n| n.name == "consumer")
        .unwrap();
    let mut req = request(ArchitectureView::Components);
    req.container = Some(consumer.id.clone());
    let components = diagram_at(project.path(), req, &config);
    assert!(
        components
            .nodes
            .iter()
            .flat_map(|n| &n.evidence)
            .any(|e| e.path.as_deref() == Some("src/bin/consumer/helper.rs"))
    );
    assert!(
        components
            .nodes
            .iter()
            .flat_map(|n| &n.evidence)
            .all(|e| e.path.as_deref() != Some("src/bin/producer.rs"))
    );

    let manifest = project.path().join("Cargo.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        original.replace("edition = \"2024\"", "edition = \"2024\"\nautobins = false"),
    )
    .unwrap();
    let explicit = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    assert_eq!(explicit.applications.len(), 1);
    assert_eq!(explicit.applications[0].name, "exporter");
}

#[test]
fn c4_follows_transitive_libraries_without_crossing_into_another_executable() {
    let project = project_with_files(&[
        (
            "app/package.json",
            r#"{"name":"app","devDependencies":{"dev":"file:../libs/dev"}}"#,
        ),
        (
            "app/index.ts",
            r#"
            import { outer } from "../libs/outer";
            import { bootstrap as other } from "../other";
            import express from "express";
            const app = express();
            export function bootstrap() { outer(); app.listen(3000); }
            bootstrap();
        "#,
        ),
        ("libs/outer/package.json", r#"{"name":"outer"}"#),
        (
            "libs/outer/index.ts",
            r#"import { inner } from "../inner"; export function outer() { inner(); }"#,
        ),
        ("libs/inner/package.json", r#"{"name":"inner"}"#),
        (
            "libs/inner/index.ts",
            r#"import { outer } from "../outer"; export function inner() {}"#,
        ),
        ("libs/dev/package.json", r#"{"name":"dev"}"#),
        ("libs/dev/index.ts", "export function dev() {}"),
        ("other/package.json", r#"{"name":"other"}"#),
        (
            "other/index.ts",
            r#"
            import { dev } from "../libs/dev";
            import express from "express";
            const app = express();
            export function bootstrap() { dev(); app.listen(3001); }
            bootstrap();
        "#,
        ),
    ]);
    let mut req = request(ArchitectureView::Components);
    req.container = Some("app:app".into());
    let components = diagram_at(project.path(), req, &ArchitectureConfig::default());
    for library in ["libs/outer", "libs/inner"] {
        assert!(
            components
                .nodes
                .iter()
                .any(|n| n.path.as_deref() == Some(library)),
            "{:?}",
            components.nodes
        );
    }
    assert!(components.edges.iter().any(|e| e.kind == "Imports"
        && e.from.ends_with(":libs/outer")
        && e.to.ends_with(":libs/inner")));
    assert!(
        components
            .nodes
            .iter()
            .all(|n| !matches!(n.path.as_deref(), Some("other" | "libs/dev")))
    );
}

#[test]
fn merely_reading_a_connection_name_leaves_database_access_pending() {
    let d = diagram(
        "knowledge/fullstack",
        request(ArchitectureView::Containers),
        &ArchitectureConfig::default(),
    );
    assert!(
        d.stores
            .iter()
            .all(|n| n.name != "backend-dotnet · Default")
    );
    assert!(
        d.pending.iter().any(|e| {
            e.text.contains("Database access in backend-dotnet")
                && e.path
                    .as_ref()
                    .is_some_and(|path| path.ends_with("OrderService.cs"))
        }),
        "{:?}",
        d.pending
    );
}

#[test]
fn connections_passed_to_ef_providers_are_inferred_and_can_be_excluded() {
    let project = project_with_files(&[
        (
            "Shop.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk.Web\"></Project>",
        ),
        (
            "Program.cs",
            r#"
            var builder = WebApplication.CreateBuilder(args);
            var reports = builder.Configuration.GetConnectionString("Reports") ?? throw new Exception();
            var stale = builder.Configuration.GetConnectionString("Stale");
            stale = "replacement";
            builder.Services.AddDbContext<AppDbContext>(options =>
                options.UseSqlServer(builder.Configuration.GetConnectionString("Orders")));
            builder.Services.AddDbContextPool<ReportsContext>((services, settings) => {
                settings.UseNpgsql(reports);
            });
            builder.Services.AddDbContext<OtherContext>(options => options.UseSqlServer(stale));
            builder.Services.AddDbContext<UnknownContext>(options =>
                otherOptions.UseSqlServer(builder.Configuration.GetConnectionString("WrongReceiver")));
            var app = builder.Build();
            app.Run();
        "#,
        ),
    ]);
    let mut config = ArchitectureConfig::default();
    let d = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    let names: std::collections::BTreeSet<_> = d.stores.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        ["Shop · Orders", "Shop · Reports"].into(),
        "{:?}",
        d.stores
    );
    let store = d
        .stores
        .iter()
        .find(|node| node.name == "Shop · Orders")
        .unwrap();
    assert_eq!(store.provenance, Provenance::Inferred);
    assert!(
        d.edges.iter().any(|e| e.from == "app:"
            && e.to == store.id
            && e.label == "Configures database access")
    );
    config.excluded_stores.insert(store.id.clone());
    let updated = diagram_at(
        project.path(),
        request(ArchitectureView::Containers),
        &config,
    );
    assert!(updated.stores.iter().any(|n| n.id == store.id));
    assert!(updated.nodes.iter().all(|n| n.id != store.id));
    assert!(updated.edges.iter().all(|e| e.to != store.id));
}
