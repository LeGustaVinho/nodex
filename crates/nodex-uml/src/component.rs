//! Component diagrams: the build units of a project (projects, packages, crates) — or the layers
//! of a single one — with the dependencies between them, the HTTP calls that connect a front end
//! to a back end, the interfaces they provide to each other, and the infrastructure the knowledge
//! graph shows they use (databases, caches, files, external APIs, configuration).

use std::collections::{BTreeMap, BTreeSet};

use nodex_ir::{StructEdgeKind, SymbolKind};

use crate::index::Index;
use crate::mermaid::{Ids, class_defs, label, lines};
use crate::scope::Scoped;
use crate::{UmlDiagram, UmlElement, UmlInput, UmlKind, UmlOptions, UmlRelation, empty};

const MAX_COMPONENTS: usize = 60;
const MAX_INTERFACES: usize = 12;

/// A drawn component: a build unit or a folder of one.
#[derive(Debug, Clone)]
struct Comp {
    key: String,
    name: String,
    /// Project-relative folder (`""` for the project root).
    root: String,
    language: Option<String>,
    types: u32,
    functions: u32,
    routes: u32,
    tests: u32,
    react: u32,
}

pub fn build(index: &Index, scoped: &Scoped, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut out = empty(UmlKind::Component);

    // Files of the scope's code.
    let mut files: BTreeSet<&str> = BTreeSet::new();
    for id in &scoped.symbols {
        if let Some(s) = index.get(id) {
            for o in &s.origins {
                let in_folder = scoped.folder.as_deref().is_none_or(|f| f.is_empty() || o.file.starts_with(&format!("{f}/")));
                let in_files = scoped.files.is_empty() || scoped.files.contains(&o.file);
                if in_folder && in_files {
                    files.insert(o.file.as_str());
                }
            }
        }
    }

    // Components: build units that hold scope files, else top-level folders.
    let mut roots: Vec<(String, String, Option<String>)> =
        input.units.iter().map(|u| (u.root.clone(), u.name.clone(), Some(format!("{:?}", u.language)))).collect();
    let unit_of = |file: &str, roots: &[(String, String, Option<String>)]| -> Option<usize> {
        roots
            .iter()
            .enumerate()
            .filter(|(_, (r, _, _))| r.is_empty() || file.starts_with(&format!("{r}/")))
            .max_by_key(|(_, (r, _, _))| r.len())
            .map(|(i, _)| i)
    };
    // Files outside every unit: grouped by top-level folder.
    for f in &files {
        if unit_of(f, &roots).is_none() {
            let top = f.split('/').next().unwrap_or("").to_owned();
            if f.contains('/') && !roots.iter().any(|(r, _, _)| *r == top) {
                roots.push((top.clone(), top, None));
            }
        }
    }
    let mut comps: BTreeMap<usize, Comp> = BTreeMap::new();
    let mut comp_of_file: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &files {
        let i = unit_of(f, &roots).unwrap_or(usize::MAX);
        comp_of_file.insert(f, i);
        comps.entry(i).or_insert_with(|| {
            let (root, name, language) = roots.get(i).cloned().unwrap_or_else(|| (String::new(), input.project.to_owned(), None));
            Comp { key: format!("unit:{root}"), name, root, language, types: 0, functions: 0, routes: 0, tests: 0, react: 0 }
        });
    }

    // A single unit: its layers (first-level folders) become the components.
    let mut layered = false;
    if comps.len() == 1 {
        let (&only, unit) = comps.iter().next().expect("one");
        let unit = unit.clone();
        let mut layers: BTreeMap<String, usize> = BTreeMap::new();
        let mut next = 10_000;
        for (f, c) in comp_of_file.iter_mut() {
            if *c != only {
                continue;
            }
            let rel = if unit.root.is_empty() { *f } else { f.strip_prefix(&format!("{}/", unit.root)).unwrap_or(f) };
            let layer = rel.split_once('/').map(|(head, _)| head.to_owned()).unwrap_or_default();
            let slot = *layers.entry(layer).or_insert_with(|| {
                next += 1;
                next
            });
            *c = slot;
        }
        if layers.len() > 1 {
            layered = true;
            comps.clear();
            for (layer, slot) in &layers {
                let root = match (unit.root.is_empty(), layer.is_empty()) {
                    (_, true) => unit.root.clone(),
                    (true, false) => layer.clone(),
                    (false, false) => format!("{}/{layer}", unit.root),
                };
                let name = if layer.is_empty() { format!("{} (root)", unit.name) } else { layer.clone() };
                comps.insert(*slot, Comp { key: format!("layer:{root}"), name, root, language: unit.language.clone(), types: 0, functions: 0, routes: 0, tests: 0, react: 0 });
            }
        } else {
            for c in comp_of_file.values_mut() {
                *c = only;
            }
        }
    }

    // What each component declares.
    let comp_of = |id: &str| -> Option<usize> {
        let s = index.get(id)?;
        let file = s.origins.first().map(|o| o.file.as_str())?;
        comp_of_file.get(file).copied()
    };
    for id in &scoped.symbols {
        let Some(s) = index.get(id) else { continue };
        let Some(c) = comp_of(id).and_then(|i| comps.get_mut(&i)) else { continue };
        if s.kind.is_type() {
            c.types += 1;
        } else if matches!(s.kind, SymbolKind::Function | SymbolKind::Method) && !s.flavors.iter().any(|f| f == "callback") {
            c.functions += 1;
        }
        if index.is_test(id) && (s.kind.is_type() || s.kind == SymbolKind::Function) {
            c.tests += 1;
        }
        if s.flavors.iter().any(|f| f == "react:component" || f == "vue:component" || f == "angular:component") {
            c.react += 1;
        }
    }
    for r in &index.knowledge.routes {
        if r.method == "PAGE" {
            continue;
        }
        if let Some(c) = comp_of(&r.handler).and_then(|i| comps.get_mut(&i)) {
            c.routes += 1;
        }
    }
    // Test-only components.
    let is_test_comp = |c: &Comp| c.tests > 0 && c.tests * 2 >= (c.types + c.functions).max(1) || c.name.to_ascii_lowercase().contains("test");
    if opts.hide_tests {
        let before = comps.len();
        comps.retain(|_, c| !is_test_comp(c));
        if comps.len() < before {
            out.warnings.push(format!("{} test component(s) hidden.", before - comps.len()));
        }
    }
    comps.retain(|_, c| c.types + c.functions > 0);
    if comps.is_empty() {
        out.warnings.push("No code in this scope.".into());
        out.mermaid = "flowchart LR\n  n0[\"No code in this scope\"]\n".into();
        return out;
    }
    if comps.len() > opts.limit(MAX_COMPONENTS) {
        let mut ranked: Vec<(usize, u32)> = comps.iter().map(|(i, c)| (*i, c.types + c.functions)).collect();
        ranked.sort_by_key(|r| std::cmp::Reverse(r.1));
        let keep: BTreeSet<usize> = ranked.iter().take(opts.limit(MAX_COMPONENTS)).map(|(i, _)| *i).collect();
        out.stats.hidden += (comps.len() - keep.len()) as u32;
        out.truncated = true;
        out.warnings.push(format!("{} small components are not shown.", comps.len() - keep.len()));
        comps.retain(|i, _| keep.contains(i));
    }

    let mut ids = Ids::new("n");
    let mut text = String::from("flowchart LR\n");
    for c in comps.values() {
        let id = ids.id(&c.key);
        let stereotype = if c.routes > 0 {
            "«service»"
        } else if c.react > 0 {
            "«web app»"
        } else if is_test_comp(c) {
            "«tests»"
        } else if layered {
            "«layer»"
        } else {
            "«component»"
        };
        let mut facts = Vec::new();
        if let Some(l) = &c.language {
            facts.push(l.clone());
        }
        if c.types > 0 {
            facts.push(format!("{} types", c.types));
        }
        if c.functions > 0 && c.types == 0 {
            facts.push(format!("{} function{}", c.functions, if c.functions == 1 { "" } else { "s" }));
        }
        if c.routes > 0 {
            facts.push(format!("{} routes", c.routes));
        }
        text.push_str(&format!("  {id}[\"{}\"]\n", lines(&[stereotype.to_owned(), c.name.clone(), facts.join(" · ")], 50)));
        out.elements.push(UmlElement {
            mermaid_id: id,
            symbol: (!c.root.is_empty()).then(|| format!("dir:{}", c.root)),
            label: c.name.clone(),
            kind: "component".into(),
            span: None,
            context: false,
            detail: Some(format!("{} · folder {}", stereotype, if c.root.is_empty() { "(project root)" } else { &c.root })),
        });
    }
    let drawn: BTreeMap<usize, String> = comps.iter().map(|(i, c)| (*i, ids.get(&c.key).unwrap_or_default().to_owned())).collect();
    let id_of = |sym: &str| comp_of(sym).and_then(|i| drawn.get(&i).cloned());

    // Dependencies between components, and HTTP calls.
    let mut uses: BTreeMap<(String, String), (u32, BTreeSet<&'static str>)> = BTreeMap::new();
    let mut http: BTreeMap<(String, String), HttpLink> = BTreeMap::new();
    for e in index.relations() {
        if !opts.allows(e.confidence) || matches!(e.kind, StructEdgeKind::Tests | StructEdgeKind::CaughtBy | StructEdgeKind::Throws) {
            continue;
        }
        let (Some(a), Some(b)) = (id_of(&e.from), id_of(&e.to)) else { continue };
        if a == b {
            continue;
        }
        if e.kind == StructEdgeKind::HttpCalls {
            let route = index
                .knowledge
                .routes
                .iter()
                .find(|r| r.handler == e.to)
                .map(|r| format!("{} {}", r.method, r.path))
                .unwrap_or_else(|| "HTTP".into());
            let entry = http.entry((a, b)).or_insert((0, BTreeSet::new(), None));
            entry.0 += 1;
            entry.1.insert(route);
            entry.2 = match (entry.2, e.confidence) {
                (None, c) => c,
                (Some(old), Some(c)) => Some(old.weakest(c)),
                (old, None) => old,
            };
            continue;
        }
        let entry = uses.entry((a, b)).or_default();
        entry.0 += 1;
        entry.1.insert(match e.kind {
            StructEdgeKind::Calls => "calls",
            StructEdgeKind::Inherits | StructEdgeKind::Implements => "extends",
            StructEdgeKind::ResolvesDi | StructEdgeKind::DependsOn => "injects",
            _ => "uses",
        });
    }
    // References declared in manifests but not seen in code.
    for u in input.units {
        let Some(a) = comps.values().find(|c| c.key == format!("unit:{}", u.root)).and_then(|c| ids.get(&c.key).map(str::to_owned)) else { continue };
        for r in &u.refs {
            let Some(b) = comps.values().find(|c| c.key == format!("unit:{r}")).and_then(|c| ids.get(&c.key).map(str::to_owned)) else { continue };
            uses.entry((a.clone(), b)).or_default().1.insert("references");
        }
    }
    let mut link = 0usize;
    let mut dashed_links = Vec::new();
    for ((a, b), (n, kinds)) in &uses {
        let what = if kinds.contains("references") && *n == 0 { "references".to_owned() } else if *n > 1 { format!("uses ×{n}") } else { "uses".to_owned() };
        text.push_str(&format!("  {a} -->|\"{}\"| {b}\n", label(&what, 30)));
        link += 1;
        out.relations.push(UmlRelation {
            from: a.clone(),
            to: b.clone(),
            kind: "dependency".into(),
            label: Some(what),
            confidence: None,
            evidence: Some(if *n == 0 { "declared in the manifest".into() } else { format!("{n} relations in code: {}", kinds.iter().copied().collect::<Vec<_>>().join(", ")) }),
            site: None,
            count: *n,
        });
    }
    for ((a, b), (n, routes, confidence)) in &http {
        let mut list: Vec<&String> = routes.iter().take(3).collect();
        let more = routes.len().saturating_sub(3);
        let mut lbl = list.drain(..).cloned().collect::<Vec<_>>().join(", ");
        if more > 0 {
            lbl.push_str(&format!(" +{more}"));
        }
        text.push_str(&format!("  {a} -.->|\"{}\"| {b}\n", label(&format!("HTTP {lbl}"), 70)));
        dashed_links.push((link, *confidence));
        link += 1;
        out.relations.push(UmlRelation {
            from: a.clone(),
            to: b.clone(),
            kind: "http".into(),
            label: Some(lbl),
            confidence: *confidence,
            evidence: Some(format!("{n} HTTP request(s) match routes: {}", routes.iter().cloned().collect::<Vec<_>>().join(", "))),
            site: None,
            count: *n,
        });
    }

    // Interfaces one component provides and another requires.
    let mut iface_count = 0;
    let mut provided: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for e in index.graph.edges.iter().filter(|e| matches!(e.kind, StructEdgeKind::Implements | StructEdgeKind::ResolvesDi)) {
        let (contract, implementation) = if e.kind == StructEdgeKind::Implements { (&e.to, &e.from) } else { (&e.from, &e.to) };
        let Some(c) = index.get(contract).filter(|s| s.kind.is_contract() && !s.external) else { continue };
        let Some(provider) = id_of(implementation) else { continue };
        provided.entry(c.id.clone()).or_insert_with(|| (provider, BTreeSet::new()));
    }
    for e in index.relations().filter(|e| matches!(e.kind, StructEdgeKind::References | StructEdgeKind::DependsOn | StructEdgeKind::Calls)) {
        let Some(target) = index.owner_type(&e.to) else { continue };
        let Some((provider, users)) = provided.get_mut(&target.id) else { continue };
        if let Some(user) = id_of(&e.from).filter(|u| u != provider) {
            users.insert(user);
        }
    }
    for (contract, (provider, users)) in &provided {
        if users.is_empty() || iface_count >= MAX_INTERFACES {
            continue;
        }
        iface_count += 1;
        let name = index.get(contract).map(|s| s.name.clone()).unwrap_or_default();
        let id = ids.id(&format!("iface:{contract}"));
        text.push_str(&format!("  {id}((\"{}\")):::iface\n", label(&name, 40)));
        text.push_str(&format!("  {provider} --- {id}\n"));
        for u in users {
            text.push_str(&format!("  {u} -->|\"requires\"| {id}\n"));
                out.relations.push(UmlRelation { from: u.clone(), to: id.clone(), kind: "requires".into(), label: Some("requires".into()), confidence: None, evidence: Some(format!("uses {name}, implemented in another component")), site: None, count: 1 });
        }
        out.relations.push(UmlRelation { from: provider.clone(), to: id.clone(), kind: "provides".into(), label: None, confidence: None, evidence: Some(format!("implements {name}")), site: None, count: 1 });
        out.elements.push(UmlElement {
            mermaid_id: id,
            symbol: Some(contract.clone()),
            label: name,
            kind: "interface".into(),
            span: index.get(contract).and_then(|s| s.origins.first().cloned()),
            context: false,
            detail: Some("provided interface".into()),
        });
    }

    // Infrastructure the knowledge graph shows.
    let infra = infrastructure(index, &id_of);
    let mut infra_used = false;
    for (key, (shape, name, users)) in &infra {
        if users.is_empty() {
            continue;
        }
        infra_used = true;
        let id = ids.id(key);
        let lbl = label(name, 60);
        let node = match shape {
            Shape::Database => format!("{id}[(\"{lbl}\")]"),
            Shape::External => format!("{id}{{{{\"{lbl}\"}}}}"),
            Shape::Store => format!("{id}[/\"{lbl}\"/]"),
        };
        text.push_str(&format!("  {node}:::infra\n"));
        for (user, what) in users {
            text.push_str(&format!("  {user} -->|\"{}\"| {id}\n", label(what, 40)));
                out.relations.push(UmlRelation { from: user.clone(), to: id.clone(), kind: "uses".into(), label: Some(what.clone()), confidence: None, evidence: Some(format!("{what} ({name})")), site: None, count: 1 });
        }
        out.elements.push(UmlElement { mermaid_id: id, symbol: None, label: name.clone(), kind: format!("{shape:?}").to_lowercase(), span: None, context: false, detail: None });
    }

    let mut used = vec!["comp"];
    if iface_count > 0 {
        used.push("iface");
    }
    if infra_used {
        used.push("infra");
    }
    class_defs(&mut text, &used);
    let comp_ids: Vec<&str> = out.elements.iter().filter(|e| e.kind == "component").map(|e| e.mermaid_id.as_str()).collect();
    text.push_str(&format!("  class {} comp\n", comp_ids.join(",")));
    for (i, confidence) in dashed_links {
        let color = match confidence {
            Some(nodex_ir::knowledge::Confidence::Unresolved) => "#e5534b",
            _ => "#f0883e",
        };
        text.push_str(&format!("  linkStyle {i} stroke:{color},stroke-width:2px\n"));
    }
    out.mermaid = text;
    out
}

/// HTTP requests from one component to another: count, routes reached, weakest confidence.
type HttpLink = (u32, BTreeSet<String>, Option<nodex_ir::knowledge::Confidence>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    Database,
    External,
    Store,
}

type Infra = BTreeMap<String, (Shape, String, BTreeMap<String, String>)>;

/// Databases (data models and writes), caches, files, browser storage, configuration and
/// external HTTP APIs, with the components that use them.
fn infrastructure(index: &Index, id_of: &dyn Fn(&str) -> Option<String>) -> Infra {
    let mut out: Infra = BTreeMap::new();
    let mut add = |key: &str, shape: Shape, name: String, user: Option<String>, what: &str| {
        let Some(user) = user else { return };
        let entry = out.entry(key.to_owned()).or_insert_with(|| (shape, name, BTreeMap::new()));
        let w = entry.2.entry(user).or_insert_with(|| what.to_owned());
        if !w.split(" & ").any(|x| x == what) {
            *w = format!("{w} & {what}");
        }
    };
    // Databases: per data framework, named by its tables.
    let mut tables: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut framework_of: BTreeMap<String, String> = BTreeMap::new();
    // Data frameworks per language, for code that queries without a model in sight.
    let mut by_language: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for m in &index.knowledge.models {
        let name = m.model.table.clone().unwrap_or_else(|| index.get(&m.symbol).map(|s| s.name.clone()).unwrap_or_default());
        tables.entry(m.model.framework.clone()).or_default().insert(name);
        framework_of.insert(m.symbol.clone(), m.model.framework.clone());
        if let Some(s) = index.get(&m.symbol) {
            by_language.entry(format!("{:?}", s.language)).or_default().insert(m.model.framework.clone());
        }
    }
    for e in index.graph.edges.iter().filter(|e| e.kind == StructEdgeKind::Mutates) {
        let Some(fw) = framework_of.get(&e.to) else { continue };
        add(&format!("db:{fw}"), Shape::Database, db_name(fw, tables.get(fw)), id_of(&e.from), "writes");
    }
    for i in &index.knowledge.symbols {
        let caps = |c: &str| i.capabilities.iter().any(|x| x == c);
        let user = id_of(&i.symbol);
        if caps("DB_READ") || caps("DB_WRITE") {
            // The only data framework of the same language, else an unnamed database.
            let lang = index.get(&i.symbol).map(|s| format!("{:?}", s.language)).unwrap_or_default();
            let fw = match by_language.get(&lang) {
                Some(set) if set.len() == 1 => set.iter().next().cloned().unwrap_or_default(),
                _ => "database".to_owned(),
            };
            let what = if caps("DB_WRITE") { "writes" } else { "reads" };
            add(&format!("db:{fw}"), Shape::Database, db_name(&fw, tables.get(&fw)), user.clone(), what);
        }
        if caps("CACHE") {
            add("cache", Shape::Database, "Cache".into(), user.clone(), "caches");
        }
        if caps("FILE_IO") {
            add("files", Shape::Store, "File system".into(), user.clone(), "reads/writes files");
        }
        if caps("LOCAL_STORAGE") {
            add("storage", Shape::Store, "Local storage".into(), user.clone(), "stores");
        }
    }
    for c in &index.knowledge.config {
        let what = match c.source.as_str() {
            "env" => "environment",
            "connection-string" => "connection string",
            _ => "settings",
        };
        add("config", Shape::Store, "Configuration".into(), id_of(&c.owner), what);
    }
    // Requests to hosts outside the project.
    for h in &index.knowledge.http_calls {
        let Some(host) = h.url.strip_prefix("https://").or_else(|| h.url.strip_prefix("http://")).map(|r| r.split(['/', '?']).next().unwrap_or(r).to_owned()) else { continue };
        if host.starts_with("localhost") || host.starts_with("127.0.0.1") || host.contains('{') || host.contains('$') {
            continue;
        }
        add(&format!("api:{host}"), Shape::External, format!("External API {host}"), id_of(&h.owner), "HTTP");
    }
    out
}

pub(crate) fn db_name(framework: &str, tables: Option<&BTreeSet<String>>) -> String {
    let fw = match framework {
        "efcore" => "EF Core",
        "typeorm" => "TypeORM",
        "diesel" => "diesel",
        "sea-orm" => "SeaORM",
        "prisma" => "Prisma",
        other => other,
    };
    match tables {
        Some(t) if !t.is_empty() => {
            let mut list: Vec<&str> = t.iter().map(String::as_str).take(4).collect();
            if t.len() > 4 {
                list.push("…");
            }
            format!("Database ({fw}): {}", list.join(", "))
        }
        _ if fw == "database" => "Database".to_owned(),
        _ => format!("Database ({fw})"),
    }
}
