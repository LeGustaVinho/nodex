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

/// Syntax coverage fixture (C# 1–14): outline snapshot per file.
#[test]
fn syntax_fixture_snapshot() {
    let project = open("csharp/Syntax");
    for file in SYNTAX_FILES {
        let ir = project.file_ir(file).expect("file is tracked");
        insta::assert_snapshot!(format!("syntax_{}", file.replace('.', "_")), outline_file(ir));
    }
}

const SYNTAX_FILES: [&str; 6] = ["Declarations.cs", "Statements.cs", "Expressions.cs", "Preprocessor.cs", "Advanced.cs", "Program.cs"];

fn has_opaque(e: &nodex_ir::Expr) -> bool {
    e.concept == Concept::Opaque || e.children.iter().any(has_opaque)
}

/// The syntax fixture covers C# 1–14 and must map completely: no parse diagnostics, no
/// `Opaque` statement or expression, and every declaration with a distinct symbol.
#[test]
fn syntax_fixture_is_fully_mapped() {
    let mut project = open("csharp/Syntax");
    for file in SYNTAX_FILES {
        let ir = project.file_ir(file).unwrap();
        assert!(ir.diagnostics.is_empty(), "{file}: {:?}", ir.diagnostics);
        for flow in &ir.flows {
            for n in &flow.nodes {
                assert_ne!(n.concept, Concept::Opaque, "{file} {}: `{}`", flow.symbol, n.code);
                assert!(!n.expr.as_ref().is_some_and(has_opaque), "{file} {}: opaque expression in `{}`", flow.symbol, n.code);
            }
        }
    }

    let graph = project.structure();
    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("missing symbol {id}"));
    let has = |id: &str, flavor: &str| sym(id).flavors.iter().any(|f| f == flavor);
    let decl = "Syntax.Declarations";

    // Distinct ids for static constructors and explicit interface implementations.
    assert!(sym(&format!("{decl}.Container`1..cctor()")).has_flow);
    assert!(sym(&format!("{decl}.Container`1..ctor()")).has_flow);
    assert!(has(&format!("{decl}.Container`1.IEnumerable.GetEnumerator()"), "explicit-interface"));
    // Parameter passing modes and params arrays are part of the id.
    sym(&format!("{decl}.Container`1.Name(in int, ref int, out string, params int[])"));
    // Event accessors and primary constructors.
    assert!(sym(&format!("{decl}.Container`1.Changed.add")).has_flow);
    assert!(sym(&format!("{decl}.Container`1.Changed.remove")).has_flow);
    assert!(has(&format!("{decl}.Square..ctor(double)"), "primary-constructor"));
    assert!(has(&format!("{decl}.Buffer"), "ref-struct"));
    // C# 14 extension members belong to the static class and name their receiver.
    assert!(has(&format!("{decl}.Extensions.Shout()"), "extension"));
    assert!(has(&format!("{decl}.Extensions.Shout()"), "extends:string text"));
    assert!(has(&format!("{decl}.Extensions.WordCount"), "extension"));
    assert!(!graph.symbols.iter().any(|s| s.id.contains("..ctor(string)") && s.id.contains("Extensions")));

    // Conditional compilation keeps every variant, tagged with its condition.
    assert!(has("Syntax.Preprocessor.Conditional.DebugOnly()", "#if DEBUG"));
    assert!(has("Syntax.Preprocessor.Conditional.ReleaseOnly()", "#else"));
    let run = project.flow("Syntax.Preprocessor.Conditional.Run(int, Dictionary<string, int>, object)").unwrap();
    let switch = run.nodes.iter().find(|n| n.concept == Concept::BuildSwitch).expect("build switch");
    let variants: Vec<&str> = switch.outputs.iter().filter(|p| p.id.starts_with("variant:")).map(|p| p.label.as_str()).collect();
    assert_eq!(variants, ["#if DEBUG", "#elif TRACE", "#else"]);
    assert!(run.nodes.iter().any(|n| n.concept == Concept::BuildSwitch && n.label.contains("UNITY_EDITOR")));
    // `out var found` and `is string text` declare locals that later statements read.
    let uses = |name: &str| run.nodes.iter().any(|n| n.inputs.iter().any(|p| p.id == format!("use:{name}")));
    assert!(uses("found") && uses("text"));

    // Local functions get their own Blueprint, linked from the declaring node.
    let locals_id = "Syntax.Statements.Statements.Locals()";
    let local_fn = format!("{locals_id}.localFunction(int)");
    assert!(project.flow(&local_fn).is_some());
    let locals = project.flow(locals_id).unwrap();
    assert!(locals.nodes.iter().any(|n| n.calls.iter().any(|c| c.symbol.as_deref() == Some(local_fn.as_str()))));
    // `for (;;)` is an endless loop; deconstruction declarations are never dropped.
    let loops = project.flow("Syntax.Statements.Statements.Loops(int[], Dictionary<string, int>)").unwrap();
    assert!(loops.nodes.iter().any(|n| n.concept == Concept::LoopInfinite));
    let asyncs = project.flow("Syntax.Statements.Statements.Asynchrony(IAsyncEnumerable<int>, CancellationToken)").unwrap();
    assert!(asyncs.nodes.iter().any(|n| n.label.starts_with("unpack (a, b)") && n.writes.contains(&"a".to_owned())));

    // Round 2: C# 13/14 syntax the grammar predates, operator overloads, docs, top-level code.
    let adv = "Syntax.Advanced";
    sym(&format!("{adv}.Money.op_+=(Money)"));
    sym(&format!("{adv}.Money.op_checked_+(Money, Money)"));
    sym(&format!("{adv}.Money.op_+(Money, Money)"));
    assert_eq!(sym(&format!("{adv}.Money")).doc.as_deref(), Some("A money amount with checked arithmetic."));
    assert!(has(&format!("{adv}.Widget.OnClick(object?, EventArgs)"), "async-void"));
    assert!(!sym(&format!("{adv}.Widget.MessageBeep(uint)")).has_flow, "extern methods have no body");
    assert!(has(&format!("{adv}.Inner.Hidden"), "file"));
    sym(&format!("{adv}.Widget..ctor(int)"));
    sym(&format!("{adv}.Widget.Shown"));
    sym(&format!("{adv}.Widget.Hidden"));
    let stmts = project.flow(&format!("{adv}.Advanced.Statements(Node?, int[], Span<int>, (int, int))")).unwrap();
    let assign = stmts.nodes.iter().find(|n| n.code.starts_with("node?.Value = 5")).expect("null-conditional assignment");
    assert!(assign.flavors.iter().any(|f| f == "safe-access"));
    assert!(assign.label.starts_with("if node exists"));
    assert!(stmts.nodes.iter().any(|n| n.concept == Concept::Match && n.label.starts_with("match pair")));
    // Top-level program with a local function.
    let main = graph.symbols.iter().find(|s| s.name == "Top-level statements").expect("top-level program");
    let program = project.flow(&main.id).unwrap();
    assert!(program.nodes.iter().any(|n| n.concept == Concept::Return));
    assert!(graph.symbols.iter().any(|s| s.kind == nodex_ir::SymbolKind::Function && s.name == "Describe"));
}
