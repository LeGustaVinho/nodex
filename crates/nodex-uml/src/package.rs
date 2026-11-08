//! Package diagrams: namespaces, modules or folders nested as in the Hierarchy view, with the
//! dependencies between them, their cycles and their coupling metrics.

use std::collections::{BTreeMap, BTreeSet};

use nodex_ir::{StructEdgeKind, Symbol, SymbolKind};

use crate::index::{Index, dirname, package_key};
use crate::mermaid::{Ids, class_defs, label, lines};
use crate::scope::Scoped;
use crate::{PackageGrouping, UmlDiagram, UmlElement, UmlKind, UmlOptions, UmlRelation, empty};

const MAX_PACKAGES: usize = 80;

/// One package of the tree.
#[derive(Default, Debug)]
struct Node {
    /// Full key (`Sample.Services`, `src/domain`).
    key: String,
    /// Label after compacting single-child chains (`Company.Product.Core`).
    name: String,
    /// Separator of the key's segments (`.`, `::`, `/`).
    sep: &'static str,
    children: BTreeMap<String, Node>,
    types: u32,
    functions: u32,
    abstracts: u32,
    tests: u32,
}

impl Node {
    fn own(&self) -> u32 {
        self.types + self.functions
    }
}

pub fn build(index: &Index, scoped: &Scoped, opts: &UmlOptions) -> UmlDiagram {
    let mut out = empty(UmlKind::Package);
    // Package of each symbol in the scope that declares code (types and free functions).
    let mut package_of: BTreeMap<String, (String, &'static str)> = BTreeMap::new();
    let mut counted: Vec<(&Symbol, String, &'static str)> = Vec::new();
    for id in &scoped.symbols {
        let Some(s) = index.get(id) else { continue };
        if opts.hide_tests && index.is_test(id) {
            continue;
        }
        let key = key_of(index, s, opts.grouping);
        let declares = s.kind.is_type() || matches!(s.kind, SymbolKind::Function) && !s.flavors.iter().any(|f| f == "callback" || f == "local-function");
        if declares {
            counted.push((s, key.0.clone(), key.1));
        }
        package_of.insert(id.clone(), key);
    }
    if counted.is_empty() {
        out.warnings.push("No types or functions in this scope.".into());
        out.mermaid = "flowchart TB\n  n0[\"No code in this scope\"]\n".into();
        return out;
    }
    // Tree of packages.
    let mut root = Node::default();
    for (s, key, sep) in &counted {
        let node = node_at(&mut root, key, sep);
        if s.kind.is_type() {
            node.types += 1;
            if matches!(s.kind, SymbolKind::Interface | SymbolKind::Trait) || s.flavors.iter().any(|f| f == "abstract") {
                node.abstracts += 1;
            }
        } else {
            node.functions += 1;
        }
        if index.is_test(&s.id) {
            node.tests += 1;
        }
    }
    compact(&mut root);

    // Too many packages: keep the upper levels, folding deeper ones into their ancestor.
    let mut depth = usize::MAX;
    while count_drawn(&root, depth) > opts.limit(MAX_PACKAGES) && depth > 1 {
        depth = if depth == usize::MAX { max_depth(&root) - 1 } else { depth - 1 };
    }
    if depth != usize::MAX {
        fold(&mut root, depth);
        out.truncated = true;
        out.warnings.push(format!("Packages deeper than {depth} levels are folded into their parent; pick a folder or a namespace to see them."));
    }

    // Each package key → the drawn node it belongs to (folded packages map to their ancestor).
    let mut ids = Ids::new("n");
    let mut drawn_of: BTreeMap<String, String> = BTreeMap::new();
    let mut text = String::from("flowchart TB\n");
    let mut groups = 0usize;
    for child in root.children.values() {
        emit(child, 1, &mut text, &mut ids, &mut groups, &mut drawn_of, &mut out.elements);
    }

    // Dependencies between drawn packages, from every relation of their code.
    let resolve = |id: &str| -> Option<String> {
        let key = package_of.get(id).map(|k| k.0.clone()).or_else(|| {
            let owner = index.owner_type(id).or_else(|| index.get(id))?;
            (!owner.external).then(|| key_of(index, owner, opts.grouping).0)
        })?;
        drawn_for(&drawn_of, &key)
    };
    let mut deps: BTreeMap<(String, String), (u32, BTreeSet<&'static str>)> = BTreeMap::new();
    for e in index.relations() {
        if !opts.allows(e.confidence) || matches!(e.kind, StructEdgeKind::Tests | StructEdgeKind::CaughtBy) {
            continue;
        }
        let (Some(a), Some(b)) = (resolve(&e.from), resolve(&e.to)) else { continue };
        if a == b {
            continue;
        }
        let entry = deps.entry((a, b)).or_default();
        entry.0 += 1;
        entry.1.insert(kind_word(e.kind));
    }

    // Cycles (strongly connected packages).
    let nodes: Vec<String> = out.elements.iter().map(|e| e.mermaid_id.clone()).collect();
    let cycles = tarjan(&nodes, &deps);
    let in_cycle: BTreeMap<&str, usize> = cycles.iter().enumerate().flat_map(|(i, c)| c.iter().map(move |n| (n.as_str(), i))).collect();
    for c in &cycles {
        let names: Vec<String> = c.iter().filter_map(|id| out.elements.iter().find(|e| e.mermaid_id == *id)).map(|e| e.label.clone()).collect();
        out.warnings.push(format!("Dependency cycle: {}", names.join(" → ")));
    }

    // Metrics per package: afferent/efferent coupling, instability, abstractness.
    for el in &mut out.elements {
        let ce = deps.keys().filter(|(a, _)| *a == el.mermaid_id).count();
        let ca = deps.keys().filter(|(_, b)| *b == el.mermaid_id).count();
        let instability = if ca + ce == 0 { 0.0 } else { ce as f32 / (ca + ce) as f32 };
        let extra = format!("Ca {ca} · Ce {ce} · instability {instability:.2}");
        el.detail = Some(match &el.detail {
            Some(d) => format!("{d} · {extra}"),
            None => extra,
        });
    }

    let mut cycle_links = Vec::new();
    for (i, ((a, b), (n, kinds))) in deps.iter().enumerate() {
        let what = if kinds.len() == 1 { kinds.iter().next().copied().unwrap_or("uses") } else { "uses" };
        let lbl = if *n > 1 { format!("{what} ×{n}") } else { what.to_owned() };
        text.push_str(&format!("  {a} -->|\"{}\"| {b}\n", label(&lbl, 30)));
        let cyclic = in_cycle.get(a.as_str()).is_some_and(|ca| in_cycle.get(b.as_str()) == Some(ca));
        if cyclic {
            cycle_links.push(i);
        }
        out.relations.push(UmlRelation {
            from: a.clone(),
            to: b.clone(),
            kind: if cyclic { "cycle".into() } else { "dependency".into() },
            label: Some(lbl),
            confidence: None,
            evidence: Some(format!("{n} relations: {}", kinds.iter().copied().collect::<Vec<_>>().join(", "))),
            site: None,
            count: *n,
        });
    }

    // External packages the code imports.
    if opts.include_external {
        let mut ext: BTreeMap<String, (String, u32)> = BTreeMap::new();
        for e in index.graph.edges.iter().filter(|e| e.kind == StructEdgeKind::Imports) {
            let Some(target) = index.get(&e.to).filter(|s| s.kind == SymbolKind::Package) else { continue };
            let Some(from) = resolve(&e.from) else { continue };
            let id = ids.id(&format!("ext:{}", target.id));
            let entry = ext.entry(id).or_insert_with(|| (target.name.clone(), 0));
            entry.1 += 1;
            text.push_str(&format!("  {from} -.-> {}\n", ids.get(&format!("ext:{}", target.id)).unwrap_or_default()));
        }
        if !ext.is_empty() {
            text.push_str("  subgraph external[\"External packages\"]\n");
            for (id, (name, _)) in &ext {
                text.push_str(&format!("    {id}[\"{}\"]:::ext\n", label(name, 40)));
                out.elements.push(UmlElement { mermaid_id: id.clone(), symbol: None, label: name.clone(), kind: "external".into(), span: None, context: true, detail: None });
            }
            text.push_str("  end\n");
        }
    }

    let mut used = vec!["pkg"];
    if !cycles.is_empty() {
        used.push("warn");
    }
    if opts.include_external {
        used.push("ext");
    }
    class_defs(&mut text, &used);
    let pkg_ids: Vec<&str> = out.elements.iter().filter(|e| e.kind == "package").map(|e| e.mermaid_id.as_str()).collect();
    if !pkg_ids.is_empty() {
        text.push_str(&format!("  class {} pkg\n", pkg_ids.join(",")));
    }
    let warn_ids: Vec<&str> = in_cycle.keys().copied().collect();
    if !warn_ids.is_empty() {
        text.push_str(&format!("  class {} warn\n", warn_ids.join(",")));
    }
    for i in cycle_links {
        text.push_str(&format!("  linkStyle {i} stroke:#e5534b,stroke-width:2px\n"));
    }
    if groups == 0 && out.elements.len() == 1 {
        out.warnings.push("The scope holds a single package; a class diagram shows more.".into());
    }
    out.mermaid = text;
    out
}

fn key_of(index: &Index, s: &Symbol, grouping: PackageGrouping) -> (String, &'static str) {
    match grouping {
        PackageGrouping::Folder => (dirname(s.origins.first().map(|o| o.file.as_str()).unwrap_or("")).to_owned(), "/"),
        PackageGrouping::Namespace => package_key(index, s),
    }
}

fn kind_word(kind: StructEdgeKind) -> &'static str {
    match kind {
        StructEdgeKind::Calls => "calls",
        StructEdgeKind::Inherits | StructEdgeKind::Implements | StructEdgeKind::Compatible => "extends",
        StructEdgeKind::Imports => "imports",
        StructEdgeKind::HttpCalls => "HTTP",
        StructEdgeKind::ResolvesDi | StructEdgeKind::DependsOn => "injects",
        StructEdgeKind::Mutates => "writes",
        StructEdgeKind::Instantiates => "creates",
        _ => "uses",
    }
}

fn node_at<'a>(root: &'a mut Node, key: &str, sep: &'static str) -> &'a mut Node {
    let mut cur = root;
    let mut path = String::new();
    let parts: Vec<&str> = if key.is_empty() { vec!["(root)"] } else { key.split(sep).filter(|p| !p.is_empty()).collect() };
    for part in parts {
        path = if path.is_empty() { part.to_owned() } else { format!("{path}{sep}{part}") };
        let p = path.clone();
        cur = cur.children.entry(part.to_owned()).or_insert_with(|| Node { key: p, name: part.to_owned(), sep, ..Node::default() });
    }
    cur
}

/// `a` → `b` → `c` with nothing of their own: one package `a.b.c`.
fn compact(node: &mut Node) {
    for child in node.children.values_mut() {
        compact(child);
        while child.own() == 0 && child.children.len() == 1 {
            let (_, only) = std::mem::take(&mut child.children).into_iter().next().expect("one child");
            child.name = format!("{}{}{}", child.name, child.sep, only.name);
            child.key = only.key;
            child.children = only.children;
            child.types = only.types;
            child.functions = only.functions;
            child.abstracts = only.abstracts;
            child.tests = only.tests;
        }
    }
}

fn max_depth(node: &Node) -> usize {
    1 + node.children.values().map(max_depth).max().unwrap_or(0)
}

/// Drawn boxes when packages below `depth` are folded.
fn count_drawn(node: &Node, depth: usize) -> usize {
    node.children.values().map(|c| count_at(c, 1, depth)).sum()
}

fn count_at(node: &Node, level: usize, depth: usize) -> usize {
    if level >= depth || node.children.is_empty() {
        return 1;
    }
    usize::from(node.own() > 0) + node.children.values().map(|c| count_at(c, level + 1, depth)).sum::<usize>()
}

fn fold(node: &mut Node, depth: usize) {
    for child in node.children.values_mut() {
        fold_at(child, 1, depth);
    }
}

fn fold_at(node: &mut Node, level: usize, depth: usize) {
    if level >= depth {
        let (mut t, mut f, mut a, mut x) = (0, 0, 0, 0);
        sum(node, &mut t, &mut f, &mut a, &mut x);
        node.types = t;
        node.functions = f;
        node.abstracts = a;
        node.tests = x;
        // Folded children keep their keys reachable through the ancestor's key prefix.
        node.children.clear();
        return;
    }
    for child in node.children.values_mut() {
        fold_at(child, level + 1, depth);
    }
}

fn sum(node: &Node, t: &mut u32, f: &mut u32, a: &mut u32, x: &mut u32) {
    *t += node.types;
    *f += node.functions;
    *a += node.abstracts;
    *x += node.tests;
    for c in node.children.values() {
        sum(c, t, f, a, x);
    }
}

fn emit(node: &Node, level: usize, text: &mut String, ids: &mut Ids, groups: &mut usize, drawn_of: &mut BTreeMap<String, String>, elements: &mut Vec<UmlElement>) {
    let indent = "  ".repeat(level);
    let summary = |n: &Node| {
        let mut parts = Vec::new();
        if n.types > 0 {
            parts.push(format!("{} type{}", n.types, if n.types == 1 { "" } else { "s" }));
        }
        if n.functions > 0 {
            parts.push(format!("{} function{}", n.functions, if n.functions == 1 { "" } else { "s" }));
        }
        parts.join(" · ")
    };
    let abstractness = |n: &Node| if n.types == 0 { 0.0 } else { n.abstracts as f32 / n.types as f32 };
    if node.children.is_empty() {
        let id = ids.id(&node.key);
        text.push_str(&format!("{indent}{id}[\"{}\"]\n", lines(&[node.name.clone(), summary(node)], 60)));
        drawn_of.insert(node.key.clone(), id.clone());
        elements.push(UmlElement {
            mermaid_id: id,
            symbol: None,
            label: node.key.clone(),
            kind: "package".into(),
            span: None,
            context: false,
            detail: Some(format!("{} · abstractness {:.2}", summary(node), abstractness(node))),
        });
        return;
    }
    *groups += 1;
    let gid = format!("g{}", *groups);
    text.push_str(&format!("{indent}subgraph {gid}[\"{}\"]\n", label(&node.name, 60)));
    text.push_str(&format!("{indent}  direction TB\n"));
    if node.own() > 0 {
        let id = ids.id(&node.key);
        text.push_str(&format!("{indent}  {id}[\"{}\"]\n", lines(&[node.name.clone(), summary(node)], 60)));
        drawn_of.insert(node.key.clone(), id.clone());
        elements.push(UmlElement {
            mermaid_id: id,
            symbol: None,
            label: node.key.clone(),
            kind: "package".into(),
            span: None,
            context: false,
            detail: Some(format!("{} · abstractness {:.2}", summary(node), abstractness(node))),
        });
    }
    for child in node.children.values() {
        emit(child, level + 1, text, ids, groups, drawn_of, elements);
    }
    text.push_str(&format!("{indent}end\n"));
}

/// The drawn package a key belongs to: itself, or the nearest drawn ancestor (folded packages).
fn drawn_for(drawn_of: &BTreeMap<String, String>, key: &str) -> Option<String> {
    let key = if key.is_empty() { "(root)" } else { key };
    if let Some(id) = drawn_of.get(key) {
        return Some(id.clone());
    }
    drawn_of
        .iter()
        .filter(|(k, _)| key.starts_with(k.as_str()) && key[k.len()..].starts_with(['.', ':', '/']))
        .max_by_key(|(k, _)| k.len())
        .map(|(_, id)| id.clone())
}

/// Strongly connected groups of two or more packages (Tarjan).
fn tarjan(nodes: &[String], deps: &BTreeMap<(String, String), (u32, BTreeSet<&'static str>)>) -> Vec<Vec<String>> {
    struct State<'a> {
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on: BTreeSet<&'a str>,
        next: usize,
        out: Vec<Vec<String>>,
    }
    fn visit<'a>(v: &'a str, adj: &BTreeMap<&'a str, Vec<&'a str>>, s: &mut State<'a>) {
        s.index.insert(v, s.next);
        s.low.insert(v, s.next);
        s.next += 1;
        s.stack.push(v);
        s.on.insert(v);
        for &w in adj.get(v).map(Vec::as_slice).unwrap_or(&[]) {
            if !s.index.contains_key(w) {
                visit(w, adj, s);
                let lw = s.low[w];
                let lv = s.low[v];
                s.low.insert(v, lv.min(lw));
            } else if s.on.contains(w) {
                let iw = s.index[w];
                let lv = s.low[v];
                s.low.insert(v, lv.min(iw));
            }
        }
        if s.low[v] == s.index[v] {
            let mut group = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on.remove(w);
                group.push(w.to_owned());
                if w == v {
                    break;
                }
            }
            if group.len() > 1 {
                group.sort();
                s.out.push(group);
            }
        }
    }
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (a, b) in deps.keys() {
        adj.entry(a.as_str()).or_default().push(b.as_str());
    }
    let mut s = State { index: BTreeMap::new(), low: BTreeMap::new(), stack: Vec::new(), on: BTreeSet::new(), next: 0, out: Vec::new() };
    for n in nodes {
        if !s.index.contains_key(n.as_str()) {
            visit(n, &adj, &mut s);
        }
    }
    s.out
}
