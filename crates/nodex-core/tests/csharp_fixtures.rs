//! Snapshot, coverage, stability and Rosetta tests over the C# fixtures.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::{Project, canon};
use nodex_ir::{Concept, FileIr, StructEdgeKind};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::{CSharpFrontend, lower_source};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(CSharpFrontend)];
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

#[test]
fn sample_flows_snapshot() {
    let project = open("csharp/Sample");
    for file in ["Program.cs", "Domain/Order.cs", "Services/OrderService.cs"] {
        let ir = project.file_ir(file).expect("file is tracked");
        insta::assert_snapshot!(format!("sample_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
}

#[test]
fn unity_flows_snapshot() {
    let project = open("unity");
    let ir = project.file_ir("Assets/Scripts/PlayerController.cs").expect("tracked");
    insta::assert_snapshot!("unity_player_controller", outline_file(ir));
}

#[test]
fn sample_structure_edges() {
    let mut project = open("csharp/Sample");
    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| {
        graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind)
    };
    assert!(has("Sample.Domain.Circle", "Sample.Domain.Shape", StructEdgeKind::Inherits));
    assert!(has("Sample.Domain.Shape", "Sample.Domain.IEntity", StructEdgeKind::Implements));
    assert!(has("Sample.Services.InMemoryOrderRepository", "Sample.Services.IOrderRepository", StructEdgeKind::Implements));
    assert!(has("ns:Sample.Services", "ns:Sample.Domain", StructEdgeKind::Imports));
    assert!(has("Sample.Services.OrderService._repository", "Sample.Services.IOrderRepository", StructEdgeKind::References));
    assert!(has(
        "Sample.Services.OrderService.CountPending()",
        "Sample.Services.AuditScope",
        StructEdgeKind::Instantiates
    ));
    assert!(graph.symbols.iter().any(|s| s.id == "pkg:System.Text.Json" && s.external));
    assert!(graph.symbols.iter().any(|s| s.id == "ns:System.Linq" && s.external));
}

#[test]
fn unity_structure_marks_engine_concepts() {
    let mut project = open("unity");
    let graph = project.structure();
    let player = graph.symbols.iter().find(|s| s.id == "Game.PlayerController").expect("type");
    assert!(player.flavors.iter().any(|f| f == "unity:MonoBehaviour"));
    assert!(graph.edges.iter().any(|e| e.from == "Game.PlayerController" && e.to == "ext:MonoBehaviour"));
    let speed = graph.symbols.iter().find(|s| s.id == "Game.PlayerController.speed").expect("field");
    assert!(speed.flavors.iter().any(|f| f == "unity:inspector"));
    let ir = project.file_ir("Assets/Scripts/PlayerController.cs").unwrap();
    let update = ir.flows.iter().find(|f| f.name == "Update").unwrap();
    assert_eq!(update.nodes[0].concept, Concept::EventEntry);
    let flash = ir.flows.iter().find(|f| f.name == "Flash").unwrap();
    assert!(flash.nodes.iter().any(|n| n.concept == Concept::Wait));
}

/// Every statement of every fixture lowers to a mapped concept; `Opaque` is the explicit
/// fallback and must not appear in the curated fixtures.
#[test]
fn fixtures_have_no_unmapped_statements() {
    for (root, files) in [
        ("csharp/Sample", vec!["Program.cs", "Domain/Order.cs", "Services/OrderService.cs"]),
        ("unity", vec!["Assets/Scripts/PlayerController.cs"]),
    ] {
        let project = open(root);
        for f in files {
            let ir = project.file_ir(f).unwrap();
            assert!(ir.diagnostics.is_empty(), "{f}: {:?}", ir.diagnostics);
            for flow in &ir.flows {
                for n in &flow.nodes {
                    assert_ne!(n.concept, Concept::Opaque, "{f} {}: unmapped `{}`", flow.symbol, n.code);
                }
            }
        }
    }
}

/// Inserting a statement must not change the ids of the other statements.
#[test]
fn node_ids_survive_unrelated_insertions() {
    let before = "class C { void M() { int a = 1; Foo(a); if (a > 0) { Bar(); } } }";
    let after = "class C { void M() { int a = 1; Log(); Foo(a); if (a > 0) { Bar(); } } }";
    let ids = |src: &str| -> Vec<(String, String)> {
        let ir = lower_source("C.cs", src);
        ir.flows[0].nodes.iter().map(|n| (n.code.clone(), n.id.clone())).collect()
    };
    let a = ids(before);
    let b = ids(after);
    for (code, id) in &a {
        let other = b.iter().find(|(c, _)| c == code).map(|(_, i)| i);
        assert_eq!(other, Some(id), "id of `{code}` changed");
    }
    assert_eq!(b.len(), a.len() + 1);
}

/// Syntax errors never abort lowering.
#[test]
fn broken_code_still_lowers() {
    let ir = lower_source("Broken.cs", "class C { void M() { int x = ; Foo(); } }");
    assert!(!ir.diagnostics.is_empty());
    assert!(!ir.flows.is_empty());
}

#[test]
fn rosetta_filter_map_sum_csharp() {
    let path = fixtures().join("rosetta/filter-map-sum");
    let text = std::fs::read_to_string(path.join("filter_map_sum.cs")).unwrap();
    let expected = std::fs::read_to_string(path.join("expected.canon")).unwrap();
    let ir = lower_source("filter_map_sum.cs", &text);
    assert_eq!(canon::canonical(&ir.flows[0]).trim(), expected.replace("\r\n", "\n").trim());
}
