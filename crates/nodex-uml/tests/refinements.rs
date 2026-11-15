use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nodex_core::Project;
use nodex_uml::architecture::{ArchitectureConfig, ArchitectureView, Layer};
use nodex_uml::refinement::{self, Inventory, Proposal, Reviewed, Suggestion, Target};
use nodex_uml::{UmlInput, UmlKind, UmlOptions, UmlScope};

fn inventory(root: &Path, target: Target, config: &ArchitectureConfig) -> Inventory {
    let mut project = Project::open(
        root,
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
    refinement::inventory(
        &UmlInput {
            graph: &graph,
            knowledge: &knowledge,
            units: &units,
            project: "Demo",
            flows: &Mutex::new(project),
            runtime: None,
        },
        &target,
        config,
    )
    .unwrap()
}

fn proposal(
    kind: &str,
    target: &str,
    value: &str,
    evidence: Vec<String>,
    members: Vec<String>,
) -> Suggestion {
    Suggestion {
        kind: kind.into(),
        target: target.into(),
        text: value.into(),
        evidence,
        members,
    }
}

fn review(base: &Inventory, suggestions: Vec<Suggestion>) -> Vec<Reviewed> {
    refinement::review(
        base,
        Proposal { suggestions },
        &base.evidence.iter().map(|e| e.id.clone()).collect(),
        &base.nodes.iter().map(|n| n.id.clone()).collect(),
        &base.edges.iter().map(|e| e.id.clone()).collect(),
        &base.candidates.iter().map(|c| c.id.clone()).collect(),
    )
}

fn arch(view: ArchitectureView) -> Target {
    Target::Architecture {
        view,
        scope: UmlScope::Project,
        container: (view == ArchitectureView::Components).then(|| "app:backend-dotnet".into()),
    }
}

#[test]
fn static_renderers_use_safe_aliases_and_keep_their_relationships() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let config = ArchitectureConfig::default();
    for kind in [
        UmlKind::Class,
        UmlKind::Package,
        UmlKind::Component,
        UmlKind::Activity,
        UmlKind::Sequence,
        UmlKind::State,
        UmlKind::UseCase,
    ] {
        let root = if kind == UmlKind::State {
            "knowledge/uml-states"
        } else {
            "knowledge/fullstack"
        };
        let base = inventory(
            &fixtures.join(root),
            Target::Uml {
                diagram: kind,
                scope: UmlScope::Project,
                options: UmlOptions::default(),
            },
            &config,
        );
        assert!(!base.nodes.is_empty(), "{kind:?}");
        let node = &base.nodes[0];
        let label = "Orders <script> \"\nparticipant intrusion ; | #";
        let edits = review(
            &base,
            vec![proposal(
                "label",
                &node.id,
                label,
                node.evidence.clone(),
                vec![],
            )],
        );
        let refined = refinement::apply(&base, &edits, &BTreeSet::from([0]))
            .unwrap()
            .uml
            .unwrap();
        let original = base.uml.unwrap();
        assert_eq!(original.relations, refined.relations, "{kind:?}");
        assert_eq!(
            original.mermaid.lines().count(),
            refined.mermaid.lines().count(),
            "{kind:?}"
        );
        assert!(
            refined.mermaid.contains("Orders #lt;script#gt;"),
            "{kind:?}: {}",
            refined.mermaid
        );
        assert!(!refined.mermaid.contains("<script>"));
    }
    for target in [
        Target::Packages {
            scope: UmlScope::Project,
        },
        arch(ArchitectureView::Layers),
        arch(ArchitectureView::Context),
        arch(ArchitectureView::Containers),
        arch(ArchitectureView::Components),
    ] {
        let base = inventory(&fixtures.join("knowledge/fullstack"), target, &config);
        assert!(!base.nodes.is_empty());
        let edits = review(
            &base,
            vec![proposal("summary", "", "Overview", vec![], vec![])],
        );
        let refined = refinement::apply(&base, &edits, &BTreeSet::from([0])).unwrap();
        assert_eq!(refined.edges.len(), base.edges.len());
    }
}

#[test]
fn cross_folder_groups_keep_citable_code_and_manual_layers_win() {
    let tmp = tempfile::tempdir().unwrap();
    for (folder, ty) in [
        ("unitsA", "Order"),
        ("unitsB", "Payment"),
        ("locked", "Settings"),
    ] {
        fs::create_dir(tmp.path().join(folder)).unwrap();
        fs::write(
            tmp.path().join(folder).join("Logic.cs"),
            format!("namespace Demo; public class {ty} {{ public void Approve() {{ }} }}"),
        )
        .unwrap();
    }
    let mut config = ArchitectureConfig::default();
    config.layers.insert("locked".into(), Layer::Domain);
    let base = inventory(tmp.path(), arch(ArchitectureView::Layers), &config);
    let get = |path: &str| {
        base.nodes
            .iter()
            .find(|n| n.id == format!("module:{path}"))
            .unwrap()
    };
    let (a, b, locked) = (get("unitsA"), get("unitsB"), get("locked"));
    let edits = review(
        &base,
        vec![
            proposal(
                "layer",
                &locked.id,
                "Application",
                locked.evidence.clone(),
                vec![],
            ),
            proposal(
                "group",
                "group:approval",
                "Approval",
                a.evidence.clone(),
                vec![a.id.clone(), b.id.clone()],
            ),
            proposal(
                "group",
                "group:purchase",
                "Purchase",
                b.evidence.clone(),
                vec!["group:approval".into()],
            ),
            proposal("label", "invented", "Invented", vec![], vec![]),
        ],
    );
    assert!(edits[0].error.is_some() && edits[3].error.is_some());
    let refined = refinement::apply(&base, &edits, &BTreeSet::from([1, 2])).unwrap();
    for (node, parent) in [
        (&a.id[..], "group:approval"),
        ("group:approval", "group:purchase"),
        ("group:purchase", "layer:Unclassified"),
    ] {
        assert_eq!(
            refined
                .nodes
                .iter()
                .find(|n| n.id == node)
                .unwrap()
                .parent
                .as_deref(),
            Some(parent)
        );
    }
    let edits = review(
        &base,
        vec![
            proposal(
                "group",
                "group:a",
                "A",
                a.evidence.clone(),
                vec!["group:b".into(), a.id.clone()],
            ),
            proposal(
                "group",
                "group:b",
                "B",
                a.evidence.clone(),
                vec!["group:a".into()],
            ),
        ],
    );
    assert!(edits.iter().all(|r| r.error.is_some()));
}

#[test]
fn c4_resolves_only_a_cited_http_candidate_and_keeps_database_uncertainty() {
    let tmp = tempfile::tempdir().unwrap();
    let files = [
        (
            "web/package.json",
            r#"{"name":"web","dependencies":{"react-dom":"*"}}"#,
        ),
        (
            "web/src/main.tsx",
            r#"import { createRoot } from "react-dom/client";
export function App(){ fetch("/api/orders"); return <div/>; }
createRoot(document.getElementById("root")!).render(<App />);"#,
        ),
        (
            "orders/package.json",
            r#"{"name":"orders","dependencies":{"express":"*"}}"#,
        ),
        (
            "orders/src/index.ts",
            r#"import express from "express";
const app = express();
app.get("/api/orders", orders);
export function orders(req: any, res: any) { res.json([]); }
export function startup() { app.listen(3000); } startup();"#,
        ),
        (
            "replica/package.json",
            r#"{"name":"replica","dependencies":{"express":"*"}}"#,
        ),
        (
            "replica/src/index.ts",
            r#"import express from "express";
const app = express();
app.get("/api/orders", orders);
export function orders(req: any, res: any) { res.json([]); }
export function startup() { app.listen(3001); } startup();"#,
        ),
    ];
    for (path, source) in files {
        let path = tmp.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    let base = inventory(
        tmp.path(),
        arch(ArchitectureView::Containers),
        &ArchitectureConfig::default(),
    );
    assert_eq!(base.candidates.len(), 2, "{:?}", base.pending);
    let a = &base.candidates[0];
    let b = &base.candidates[1];
    let edits = review(
        &base,
        vec![
            proposal(
                "association",
                &a.id,
                "Serves orders",
                a.edge.evidence.clone(),
                vec![],
            ),
            proposal(
                "association",
                &b.id,
                "Serves orders",
                b.edge.evidence.clone(),
                vec![],
            ),
            proposal(
                "association",
                "store:guess",
                "Database",
                a.edge.evidence.clone(),
                vec![],
            ),
            proposal(
                "association",
                &b.id,
                "Missing route evidence",
                vec![b.edge.evidence[0].clone()],
                vec![],
            ),
        ],
    );
    assert!(edits[2].error.is_some() && edits[3].error.is_some());
    assert!(refinement::apply(&base, &edits, &BTreeSet::from([0, 1])).is_err());
    let refined = refinement::apply(&base, &edits, &BTreeSet::from([0])).unwrap();
    let edge = refined.edges.iter().find(|e| e.id == a.id).unwrap();
    assert_eq!(edge.label, "GET /api/orders");
    assert_eq!(edge.confidence, "Inferred");
    assert_eq!(edge.from, a.edge.from);
    assert_eq!(edge.to, a.edge.to);

    let fullstack =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/knowledge/fullstack");
    let base = inventory(
        &fullstack,
        arch(ArchitectureView::Containers),
        &ArchitectureConfig::default(),
    );
    assert!(
        base.evidence
            .iter()
            .any(|e| base.pending.contains(&e.id) && e.text.contains("Database access"))
    );
    assert!(base.candidates.iter().all(|c| c.edge.kind == "HTTP"));
}
