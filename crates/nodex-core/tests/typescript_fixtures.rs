//! Snapshot, coverage, stability and Rosetta tests over the TypeScript/JavaScript fixtures.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::{Project, canon};
use nodex_ir::{Concept, Expr, FileIr, StructEdgeKind, SymbolKind};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_typescript::{TypeScriptFrontend, lower_source};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn open(rel: &str) -> Project {
    let frontends: Vec<Arc<dyn LanguageFrontend>> = vec![Arc::new(TypeScriptFrontend)];
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

fn has_opaque(e: &Expr) -> bool {
    e.concept == Concept::Opaque || e.children.iter().any(has_opaque)
}

#[test]
fn compiler_configuration_changes_invalidate_typescript_analysis() {
    use nodex_ir::{Language, semantic::{SemanticFacts, SemanticState}};

    let mut project = open("ts/Sample");
    let facts = || SemanticFacts {
        configuration_files: vec!["compiler-options.json".into()],
        ..Default::default()
    };
    project.apply_semantic(Language::Typescript, facts());
    let revision = project.source_revision();
    let config = project.root().join("tsconfig.json");
    assert!(config.is_file());

    assert_eq!(project.refresh_path(&config).as_deref(), Some("tsconfig.json"));
    assert_eq!(project.source_revision(), revision + 1);
    assert_eq!(project.summary().semantic[0].state, SemanticState::Loading);
    assert!(project.refresh_path(&config).is_none(), "duplicate notifications keep the same revision");
    assert!(project.file_ir("tsconfig.json").is_none());
    project.apply_semantic(Language::Typescript, facts());
    assert_eq!(project.refresh_path(&project.root().join("compiler-options.json")).as_deref(), Some("compiler-options.json"));
    assert_eq!(project.source_revision(), revision + 2);
    assert_eq!(project.summary().semantic[0].state, SemanticState::Loading);
}

#[test]
fn sample_flows_snapshot() {
    let project = open("ts/Sample");
    for file in ["src/main.ts", "src/services/orderService.ts", "src/services/repository.ts", "src/domain/order.ts", "src/legacy/format.cjs"] {
        let ir = project.file_ir(file).unwrap_or_else(|| panic!("{file} lowered"));
        insta::assert_snapshot!(format!("ts_sample_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
}

#[test]
fn sample_structure_edges() {
    let mut project = open("ts/Sample");
    let graph = project.structure();
    let has = |from: &str, to: &str, kind: StructEdgeKind| graph.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind);
    // Modules import modules (barrels resolve to `index`) and packages.
    assert!(has("ns:src/main", "ns:src/domain/index", StructEdgeKind::Imports));
    assert!(has("ns:src/main", "ns:src/services/orderService", StructEdgeKind::Imports));
    assert!(has("ns:src/domain/index", "ns:src/domain/order", StructEdgeKind::Imports));
    assert!(has("ns:src/services/orderService", "pkg:node:events", StructEdgeKind::Imports));
    // Inheritance and implementation.
    assert!(has("src/domain/shapes.Circle", "src/domain/shapes.Shape", StructEdgeKind::Inherits));
    assert!(has("src/domain/shapes.Shape", "src/domain/order.Entity", StructEdgeKind::Implements));
    assert!(has("src/services/repository.InMemoryOrderRepository", "src/services/repository.OrderRepository", StructEdgeKind::Implements));
    assert!(has("src/services/orderService.OrderService", "ext:EventEmitter", StructEdgeKind::Inherits));
    // Creation and usage.
    assert!(has("src/main.main", "src/services/orderService.OrderService", StructEdgeKind::Instantiates));
    assert!(has("src/services/orderService.OrderService.repository", "src/services/repository.OrderRepository", StructEdgeKind::References));
    // Declared packages.
    assert!(graph.symbols.iter().any(|s| s.id == "pkg:typescript" && s.external));

    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} exists"));
    assert_eq!(sym("ns:src/domain/index").kind, SymbolKind::Module);
    assert!(sym("ns:src/domain/index").flavors.iter().any(|f| f == "barrel"));
    assert!(sym("ns:src/legacy/format").flavors.iter().any(|f| f == "commonjs"));
    assert_eq!(sym("src/domain/shapes.OrderEvent").kind, SymbolKind::Union);
    assert!(sym("src/domain/order.Entity").flavors.iter().any(|f| f == "type-only"));
    assert_eq!(sym("src/domain/order.Order").doc.as_deref(), Some("A customer order. Immutable: changes create a copy."));
    assert!(sym("src/services/repository.InMemoryOrderRepository.replace").has_flow);
}

#[test]
fn sample_uses_the_shared_vocabulary() {
    let mut project = open("ts/Sample");
    let ship = project.flow("src/services/orderService.OrderService.ship").expect("ship flow");
    assert!(ship.nodes.iter().any(|n| n.concept == Concept::Wait), "await sleep(10) is a Wait");
    assert!(ship.nodes.iter().any(|n| n.concept == Concept::Emit), "this.emit is an Emit");
    let main = project.flow("src/main.main").expect("main flow");
    assert!(main.nodes.iter().any(|n| n.concept == Concept::Subscribe));
    assert!(main.nodes.iter().any(|n| n.concept == Concept::Try));
    let pipeline = project.flow("src/services/orderService.OrderService.totalForCustomerPipeline").expect("pipeline flow");
    let ret = pipeline.nodes.iter().find(|n| n.concept == Concept::Return).expect("return");
    assert_eq!(ret.expr.as_ref().map(|e| e.concept), Some(Concept::Pipeline));
}

#[test]
fn fixtures_have_no_unmapped_statements() {
    let project = open("ts/Sample");
    for (file, ir) in project.files() {
        assert!(ir.diagnostics.is_empty(), "{file}: {:?}", ir.diagnostics);
        for flow in &ir.flows {
            for n in &flow.nodes {
                assert_ne!(n.concept, Concept::Opaque, "{file} {}: unmapped `{}`", flow.symbol, n.code);
            }
        }
    }
}

/// Inserting a statement must not change the ids of the other statements.
#[test]
fn node_ids_survive_unrelated_insertions() {
    let before = "function m() { let a = 1; foo(a); if (a > 0) { bar(); } }";
    let after = "function m() { let a = 1; log(); foo(a); if (a > 0) { bar(); } }";
    let ids = |src: &str| -> Vec<(String, String)> {
        let ir = lower_source("c.ts", src);
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
    let ir = lower_source("broken.ts", "function m() { let x = ; foo(); }");
    assert!(!ir.diagnostics.is_empty());
    assert!(!ir.flows.is_empty());
}

/// Same algorithm, same picture: the TypeScript version matches the C# canonical form.
#[test]
fn rosetta_filter_map_sum_typescript() {
    let path = fixtures().join("rosetta/filter-map-sum");
    let expected = std::fs::read_to_string(path.join("expected.canon")).unwrap();
    let ts = lower_source("filter_map_sum.ts", &std::fs::read_to_string(path.join("filter_map_sum.ts")).unwrap());
    let cs = nodex_lang_csharp::lower_source("filter_map_sum.cs", &std::fs::read_to_string(path.join("filter_map_sum.cs")).unwrap());
    assert_eq!(canon::canonical(&ts.flows[0]).trim(), expected.replace("\r\n", "\n").trim());
    assert_eq!(canon::canonical(&ts.flows[0]), canon::canonical(&cs.flows[0]));
}

/// Operations Rosetta: the same logic in C# and TypeScript draws the same operation nodes.
#[test]
fn rosetta_operations_match_across_languages() {
    let path = fixtures().join("rosetta/operations");
    let expected = std::fs::read_to_string(path.join("expected.ops")).unwrap();
    let mut ts = lower_source("operations.ts", &std::fs::read_to_string(path.join("operations.ts")).unwrap());
    let mut cs = nodex_lang_csharp::lower_source("operations.cs", &std::fs::read_to_string(path.join("operations.cs")).unwrap());
    let ts = &mut ts.flows[0];
    let cs = &mut cs.flows[0];
    nodex_core::operations::recognize_flow(ts);
    nodex_core::operations::recognize_flow(cs);
    assert_eq!(canon::canonical(ts), canon::canonical(cs), "same statements");
    assert_eq!(canon::operations(ts), canon::operations(cs), "same operations");
    assert_eq!(canon::operations(ts).trim(), expected.replace("\r\n", "\n").trim());
    // The existing Rosetta program: `n % 2 === 0` and `total += n * n` in both.
    let path = fixtures().join("rosetta/filter-map-sum");
    let mut ts = lower_source("filter_map_sum.ts", &std::fs::read_to_string(path.join("filter_map_sum.ts")).unwrap());
    let mut cs = nodex_lang_csharp::lower_source("filter_map_sum.cs", &std::fs::read_to_string(path.join("filter_map_sum.cs")).unwrap());
    nodex_core::operations::recognize_flow(&mut ts.flows[0]);
    nodex_core::operations::recognize_flow(&mut cs.flows[0]);
    let ops = canon::operations(&ts.flows[0]);
    assert_eq!(ops, canon::operations(&cs.flows[0]));
    assert_eq!(ops.trim(), std::fs::read_to_string(path.join("expected.ops")).unwrap().replace("\r\n", "\n").trim());
}

/// Syntax coverage fixture (TypeScript 5.x, TSX, JavaScript): outline snapshot per file.
#[test]
fn syntax_fixture_snapshot() {
    let project = open("ts/Syntax");
    for (file, ir) in project.files() {
        insta::assert_snapshot!(format!("ts_syntax_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
}

#[test]
fn syntax_fixture_is_fully_mapped() {
    let mut project = open("ts/Syntax");
    let files: Vec<String> = project.files().map(|(f, _)| f.to_owned()).collect();
    assert!(files.len() >= 10, "{files:?}");
    for (file, ir) in project.files() {
        assert!(ir.diagnostics.is_empty(), "{file}: {:?}", ir.diagnostics);
        for flow in &ir.flows {
            for n in &flow.nodes {
                assert_ne!(n.concept, Concept::Opaque, "{file} {}: `{}`", flow.symbol, n.code);
                assert!(!n.expr.as_ref().is_some_and(has_opaque), "{file} {}: opaque expression in `{}`", flow.symbol, n.code);
            }
        }
    }

    let graph = project.structure();
    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} exists"));
    let has = |id: &str, flavor: &str| sym(id).flavors.iter().any(|f| f == flavor);
    let d = "src/declarations";
    assert!(has(&format!("{d}.Base"), "attr:sealed"));
    assert!(has(&format!("{d}.Base"), "abstract"));
    assert!(has(&format!("{d}.Base.describe"), "attr:logged"));
    assert!(has(&format!("{d}.Base.name"), "parameter-property"));
    assert!(has(&format!("{d}.Base.#secret"), "private"));
    assert!(sym(&format!("{d}.Base.secret.get")).has_flow);
    assert!(sym(&format!("{d}.Base.secret.set")).has_flow);
    assert!(sym(&format!("{d}.Base.static")).has_flow);
    assert!(has(&format!("{d}.Square.onResize"), "bound"));
    assert!(has(&format!("{d}.overload"), "overloaded"));
    assert_eq!(sym(&format!("{d}.overload")).origins.len(), 3, "two overload signatures and their implementation");
    assert!(has(&format!("{d}.counter"), "generator"));
    assert!(has(&format!("{d}.Shape"), "type-only"));
    assert_eq!(sym(&format!("{d}.Shape")).origins.len(), 2, "declaration merging");
    assert_eq!(sym(&format!("{d}.Result")).kind, SymbolKind::Union);
    assert_eq!(sym(&format!("{d}.Id")).kind, SymbolKind::TypeAlias);
    assert_eq!(sym(&format!("{d}.Geometry.Nested")).kind, SymbolKind::Namespace);
    assert!(sym(&format!("{d}.default.run")).has_flow);
    assert!(sym("src/statements.nested.helper").has_flow, "local function Blueprint");
    assert!(sym("src/statements.nested.square").has_flow, "local arrow function Blueprint");
    assert!(sym("src/legacy.Counter.increment").has_flow, "prototype method Blueprint");
    assert!(sym("src/components.App.handleClick").has_flow);
    assert!(has("ns:src/legacy", "javascript"));
    assert!(has("ns:src/types", "declarations"));
    assert!(has("ns:src/modules/index", "barrel"));
    assert!(graph.edges.iter().any(|e| e.from == "ns:src/modules/index" && e.to == "ns:src/modules/b" && e.kind == StructEdgeKind::Imports));
    assert!(graph.edges.iter().any(|e| e.from == "ns:src/components" && e.to == "pkg:react" && e.kind == StructEdgeKind::Imports));

    // Syntax the grammar predates, bridged on a same-length copy (prepare.rs), and recent forms.
    let m = "src/modern";
    assert!(has(&format!("{m}.Service.db"), "attr:Inject"), "parameter decorators");
    assert!(has(&format!("{m}.Service.log"), "parameter-property"));
    assert!(has(&format!("{m}.Service.toString"), "override"), "abstract override");
    assert!(sym(&format!("{m}.Holder.read")).has_flow, "variance annotations");
    assert!(has(&format!("{m}.global"), "global-augmentation"));
    assert_eq!(sym(&format!("{m}.global.Window")).kind, SymbolKind::Interface);
    let resources = project.flow(&format!("{m}.resources")).expect("resources flow");
    let loops: Vec<_> = resources.nodes.iter().filter(|n| n.concept == Concept::LoopForEach).collect();
    assert_eq!(loops.len(), 2);
    assert!(loops.iter().all(|n| n.flavors.iter().any(|f| f == "dispose-at-end")), "for (using …)");
    assert!(loops[1].flavors.iter().any(|f| f == "async"), "for (await using …)");

    // Functions inside object literals have Blueprints.
    let graph = project.structure();
    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} exists"));
    for id in ["src/objects.api.get", "src/objects.api.post", "src/objects.api.nested.deep", "src/objects.exports.run", "src/objects.default.methods.click"] {
        assert!(sym(id).has_flow, "{id} has a Blueprint");
        assert_eq!(sym(id).kind, SymbolKind::Method);
    }
    assert_eq!(sym("src/objects.api.get").parent.as_deref(), Some("src/objects.api"));

    let statements = project.flow("src/statements.statements").expect("statements flow");
    let concepts: Vec<Concept> = statements.nodes.iter().map(|n| n.concept).collect();
    for c in [Concept::LoopRange, Concept::LoopForEach, Concept::LoopInfinite, Concept::LoopWhile, Concept::Match, Concept::Try, Concept::Destructure, Concept::Break, Concept::Continue] {
        assert!(concepts.contains(&c), "statements uses {c:?}");
    }
    assert!(statements.nodes.iter().any(|n| n.flavors.iter().any(|f| f == "fallthrough")));
    assert!(statements.nodes.iter().any(|n| n.flavors.iter().any(|f| f == "loose-equality")));
    assert!(statements.nodes.iter().any(|n| n.flavors.iter().any(|f| f == "dispose-at-end")));
    let expressions = project.flow("src/expressions.expressions").expect("expressions flow");
    let kinds = |c: Concept| expressions.nodes.iter().filter(|n| n.concept == c).count();
    assert_eq!(kinds(Concept::Subscribe), 4);
    assert_eq!(kinds(Concept::Emit), 1);
    let waits = project.flow("src/statements.waits").expect("waits flow");
    assert_eq!(waits.nodes.iter().filter(|n| n.concept == Concept::Wait).count(), 2);
}

/// React, Vue, Angular and Node.js: framework-called code appears as event entries, callbacks
/// handed to a framework have their own Blueprints, framework calls say what they mean.
#[test]
fn frameworks_fixture() {
    let mut project = open("ts/Frameworks");
    for (file, ir) in project.files() {
        assert!(ir.diagnostics.is_empty(), "{file}: {:?}", ir.diagnostics);
        for flow in &ir.flows {
            for n in &flow.nodes {
                assert_ne!(n.concept, Concept::Opaque, "{file} {}: `{}`", flow.symbol, n.code);
            }
        }
        insta::assert_snapshot!(format!("ts_frameworks_{}", file.replace(['/', '.'], "_")), outline_file(ir));
    }
    let graph = project.structure();
    let sym = |id: &str| graph.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} exists")).clone();
    let has = |id: &str, flavor: &str| sym(id).flavors.iter().any(|f| f == flavor);
    let entry = |p: &mut Project, id: &str| {
        let f = p.flow(id).unwrap_or_else(|| panic!("{id} flow"));
        (f.nodes[0].concept, f.nodes[0].label.clone())
    };

    // React
    let r = "src/react/Counter";
    assert!(has(&format!("{r}.Counter"), "react:component"));
    assert!(has(&format!("{r}.useTicker"), "react:hook"));
    assert!(has(&format!("{r}.Row"), "react:memo"));
    assert_eq!(entry(&mut project, &format!("{r}.Counter")).0, Concept::EventEntry);
    assert!(entry(&mut project, &format!("{r}.Counter.effect")).1.contains("when count changes"));
    let counter = project.flow(&format!("{r}.Counter")).unwrap();
    let flavored = |f: &str| counter.nodes.iter().any(|n| n.flavors.iter().any(|x| x == f));
    for f in ["react:state", "react:ref", "react:context", "react:memo", "react:effect"] {
        assert!(flavored(f), "Counter uses {f}");
    }
    let effect = counter.nodes.iter().find(|n| n.flavors.iter().any(|f| f == "react:effect")).unwrap();
    assert_eq!(effect.concept, Concept::Subscribe);
    assert_eq!(effect.calls[0].symbol.as_deref(), Some(format!("{r}.Counter.effect").as_str()));
    let increment = project.flow(&format!("{r}.Counter.increment")).unwrap();
    assert!(increment.nodes.iter().any(|n| n.flavors.iter().any(|f| f == "react:set-state")));
    assert_eq!(entry(&mut project, "src/react/Clock.Clock.componentDidMount").0, Concept::EventEntry);

    // Vue
    assert!(has("ns:src/vue/TodoList", "vue:component"));
    assert!(entry(&mut project, "src/vue/TodoList.default.mounted").1.starts_with("Vue calls mounted"));
    assert!(has("src/vue/TodoList.default.computed.visible", "vue:computed"));
    assert!(entry(&mut project, "src/vue/TodoList.default.watch.todos").1.contains("when todos changes"));
    assert!(entry(&mut project, "src/vue/TodoList.default.methods.add").1.contains("'keyup'"), "template handler");
    let setup = project.flow("src/vue/Search.(module)").expect("script setup flow");
    assert_eq!(setup.nodes[0].concept, Concept::EventEntry);
    for f in ["vue:props", "vue:emits", "vue:ref", "vue:computed", "vue:watch", "vue:lifecycle"] {
        assert!(setup.nodes.iter().any(|n| n.flavors.iter().any(|x| x == f)), "setup uses {f}");
    }
    assert!(entry(&mut project, "src/vue/Search.pick").1.contains("'click'"));
    assert!(graph.edges.iter().any(|e| e.from == "ns:src/vue/Search" && e.to == "pkg:vue"));

    // Angular
    let a = "src/angular/hero-list.component.HeroListComponent";
    assert!(has(a, "angular:component"));
    assert!(has(&format!("{a}.limit"), "angular:input"));
    assert!(has(&format!("{a}.chosen"), "angular:output"));
    assert!(has(&format!("{a}.service"), "angular:inject"));
    assert!(has("src/angular/hero.service.HeroService", "angular:service"));
    assert!(has("src/angular/hero.service.HeroService.http", "angular:inject"));
    assert!(has("src/angular/hero.service.HeroService.selected", "angular:signal"));
    assert!(entry(&mut project, &format!("{a}.ngOnInit")).1.starts_with("Angular calls ngOnInit"));
    assert!(entry(&mut project, &format!("{a}.clear")).1.contains("window:keydown.escape"));
    assert!(entry(&mut project, &format!("{a}.choose")).1.contains("'click'"));
    let heroes = project.flow("src/angular/hero.service.HeroService.heroes").unwrap();
    let ret = heroes.nodes.iter().find(|n| n.concept == Concept::Return).unwrap();
    assert_eq!(ret.expr.as_ref().map(|e| e.concept), Some(Concept::Pipeline), "RxJS pipe");

    // Node.js
    let n = "src/node/server.(module)";
    assert!(entry(&mut project, &format!("{n}.GET /users/:id")).1.contains("GET /users/:id request"));
    assert!(sym(&format!("{n}.middleware")).has_flow);
    assert!(sym(&format!("{n}.on 'SIGINT'")).has_flow);
    let top = project.flow(n).unwrap();
    let routes = top.nodes.iter().filter(|x| x.flavors.iter().any(|f| f == "node:route")).count();
    assert_eq!(routes, 3);
    let sigint = project.flow(&format!("{n}.on 'SIGINT'")).unwrap();
    assert!(sigint.nodes.iter().any(|x| x.flavors.iter().any(|f| f == "node:exit") && x.concept == Concept::Return));
}
