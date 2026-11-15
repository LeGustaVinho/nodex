use std::path::PathBuf;
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
    let mut project = Project::open(
        &folder,
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
fn named_database_is_inferred_and_can_be_removed_from_the_boundary() {
    let project = "knowledge/fullstack";
    let mut config = ArchitectureConfig::default();
    let d = diagram(project, request(ArchitectureView::Containers), &config);
    let store = d
        .stores
        .iter()
        .find(|node| node.name == "backend-dotnet · Default")
        .unwrap();
    assert_eq!(store.provenance, Provenance::Inferred);
    assert!(
        d.edges
            .iter()
            .any(|e| e.from == "app:backend-dotnet" && e.to == store.id)
    );
    config.excluded_stores.insert(store.id.clone());
    let updated = diagram(project, request(ArchitectureView::Containers), &config);
    assert!(updated.stores.iter().any(|n| n.id == store.id));
    assert!(updated.nodes.iter().all(|n| n.id != store.id));
    assert!(updated.edges.iter().all(|e| e.to != store.id));
}
