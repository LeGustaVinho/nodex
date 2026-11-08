//! Project-wide knowledge: links the per-file [`FileKnowledge`] facts and what flow graphs show
//! (complexity, throws) into knowledge edges and a [`ProjectKnowledge`] summary.
//!
//! The derived passes were ported from CodeGraph (`crates/semantic/src/global.rs`). Structural
//! relations (calls, inheritance) are not guessed here: they come from the linker and from the
//! semantic providers.
//!
//! [`FileKnowledge`]: nodex_ir::knowledge::FileKnowledge

pub mod flows;
pub mod routes;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nodex_ir::knowledge::{ModelInfo, ProjectKnowledge, ResolvedRoute, SymbolInfo};
use nodex_ir::{FileIr, Language, StructEdge, StructEdgeKind, StructureGraph, Symbol, SymbolId, SymbolKind};

/// Exact construction targets reported by semantic providers: file → (start, end, type).
pub type Constructs = HashMap<String, Vec<(u32, u32, SymbolId)>>;

/// What [`build`] adds to a structure graph.
#[derive(Default)]
pub struct Knowledge {
    pub edges: Vec<StructEdge>,
    /// External symbols the edges point to (`ext:InvalidOperationException`).
    pub externals: Vec<Symbol>,
    pub project: ProjectKnowledge,
}

/// Derives knowledge from the linked `graph` and the files' facts and flows.
pub fn build(graph: &StructureGraph, files: &[&FileIr], constructs: &Constructs) -> Knowledge {
    let types = TypeLookup::new(graph);
    let symbols: HashMap<&str, &Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut out = Knowledge::default();
    let mut externals: BTreeMap<String, Symbol> = BTreeMap::new();
    let mut edges: BTreeMap<(String, String, StructEdgeKind), StructEdge> = BTreeMap::new();
    let mut infos: BTreeMap<SymbolId, SymbolInfo> = BTreeMap::new();
    let mut caps: BTreeMap<SymbolId, BTreeSet<&'static str>> = BTreeMap::new();

    for file in files {
        // Flows: complexity and throws.
        for flow in &file.flows {
            let entry = infos.entry(flow.symbol.clone()).or_insert_with(|| info(&flow.symbol));
            entry.complexity = flows::complexity(flow);
            let thrown = flows::throws(flow);
            if !thrown.is_empty() {
                caps.entry(flow.symbol.clone()).or_default().insert("THROWS");
            }
            for t in thrown {
                let Some(node) = flow.node(&t.node) else { continue };
                // The compiler's target for the `new` in this statement whose name matches.
                let exact = t.ty.as_deref().and_then(|ty| {
                    let want = simple_name(ty);
                    constructs.get(&node.span.file)?.iter().find_map(|(s, e, id)| {
                        let inside = node.span.start_byte <= *s && *e <= node.span.end_byte;
                        let named = symbols.get(id.as_str()).is_some_and(|sym| simple_name(&sym.name) == want);
                        (inside && named).then(|| id.clone())
                    })
                });
                let target = match (exact, &t.ty) {
                    (Some(id), _) => id,
                    (None, Some(ty)) => types.resolve(ty, namespace_of(&symbols, &flow.symbol).as_deref()).unwrap_or_else(|| {
                        let id = format!("ext:{ty}");
                        externals.entry(id.clone()).or_insert_with(|| external_class(&id, ty, file.language));
                        id
                    }),
                    (None, None) => continue,
                };
                let key = (flow.symbol.clone(), target.clone(), StructEdgeKind::Throws);
                edges.entry(key).or_insert_with(|| {
                    StructEdge::new(flow.symbol.clone(), target, StructEdgeKind::Throws)
                        .with_evidence(format!("`{}`", node.code))
                        .with_site(node.span.clone())
                });
            }
        }

        // Facts written by the frontend.
        let k = &file.knowledge;
        for s in &k.symbols {
            let entry = infos.entry(s.symbol.clone()).or_insert_with(|| info(&s.symbol));
            if entry.entry.is_none() {
                entry.entry = s.entry.clone();
            }
            entry.test |= s.test;
            if let Some(model) = &s.data_model {
                out.project.models.push(ModelInfo { symbol: s.symbol.clone(), model: model.clone() });
            }
        }
        for c in &k.config {
            caps.entry(c.owner.clone()).or_default().insert("CONFIG_READ");
        }
        for h in &k.http_calls {
            caps.entry(h.owner.clone()).or_default().insert("NETWORK_IO");
        }
        for m in &k.mutations {
            let tag = match m.kind.as_str() {
                "database" => "DB_WRITE",
                "cache" => "CACHE",
                "storage" => "LOCAL_STORAGE",
                _ => "STATE_WRITE",
            };
            caps.entry(m.owner.clone()).or_default().insert(tag);
        }
        out.project.http_calls.extend(k.http_calls.iter().cloned());
        out.project.di.extend(k.di.iter().cloned());
        out.project.config.extend(k.config.iter().cloned());
    }

    out.project.routes = resolve_routes(files);
    for r in &out.project.routes {
        let entry = infos.entry(r.handler.clone()).or_insert_with(|| info(&r.handler));
        if entry.entry.is_none() {
            entry.entry = Some(nodex_ir::knowledge::EntryPoint {
                kind: nodex_ir::knowledge::EntryKind::Route,
                label: format!("{} {}", r.method, r.path),
            });
        }
    }

    for (id, info) in infos.iter_mut() {
        if let Some(sym) = symbols.get(id.as_str())
            && sym.flavors.iter().any(|f| f == "async")
        {
            caps.entry(id.clone()).or_default().insert("ASYNC");
        }
        info.capabilities = caps.remove(id).map(|c| c.into_iter().map(str::to_owned).collect()).unwrap_or_default();
    }
    // Capabilities of symbols without a flow (rare: facts on a field initializer).
    for (id, c) in caps {
        let mut i = info(&id);
        i.capabilities = c.into_iter().map(str::to_owned).collect();
        infos.insert(id, i);
    }

    out.project.symbols = infos.into_values().filter(|i| symbols.contains_key(i.symbol.as_str())).collect();
    out.project.models.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    out.edges = edges.into_values().collect();
    out.externals = externals.into_values().filter(|s| !symbols.contains_key(s.id.as_str())).collect();
    out
}

/// Applies project-wide prefixes (`global` per framework) and normalizes every route.
pub fn resolve_routes(files: &[&FileIr]) -> Vec<ResolvedRoute> {
    let mut global: HashMap<&str, &str> = HashMap::new();
    for file in files {
        for p in file.knowledge.prefixes.iter().filter(|p| p.kind == "global") {
            global.insert(p.framework.as_str(), p.prefix.as_str());
        }
    }
    let mut out: Vec<ResolvedRoute> = Vec::new();
    for file in files {
        for r in &file.knowledge.routes {
            let path = match global.get(r.framework.as_str()) {
                Some(prefix) if r.method != "PAGE" => routes::join_route(prefix, &r.path),
                _ => routes::join_route("", &r.path),
            };
            out.push(ResolvedRoute {
                handler: r.handler.clone(),
                method: r.method.clone(),
                normalized: routes::normalize_route(&path),
                path,
                framework: r.framework.clone(),
                span: r.span.clone(),
            });
        }
    }
    out.sort_by(|a, b| (&a.normalized, &a.method, &a.handler).cmp(&(&b.normalized, &b.method, &b.handler)));
    out.dedup_by(|a, b| a.normalized == b.normalized && a.method == b.method && a.handler == b.handler);
    out
}

/// Type lookup by simple name, preferring the caller's namespace.
pub struct TypeLookup {
    by_name: HashMap<String, Vec<(SymbolId, String)>>,
}

impl TypeLookup {
    pub fn new(graph: &StructureGraph) -> Self {
        let mut by_name: HashMap<String, Vec<(SymbolId, String)>> = HashMap::new();
        for s in graph.symbols.iter().filter(|s| s.kind.is_type()) {
            let simple = simple_name(&s.name);
            by_name.entry(simple).or_default().push((s.id.clone(), s.qualified_name.replace("::", ".")));
        }
        for v in by_name.values_mut() {
            v.sort();
        }
        Self { by_name }
    }

    /// The project (or already external) type named `name` (`Order`, `Domain.Order`, `Vec<T>`).
    pub fn resolve(&self, name: &str, from_ns: Option<&str>) -> Option<SymbolId> {
        let name = name.replace("::", ".");
        let name = name.split('<').next().unwrap_or(&name).trim();
        let simple = simple_name(name);
        let candidates = self.by_name.get(&simple)?;
        if name.contains('.') {
            return candidates.iter().find(|(_, q)| q.ends_with(name)).map(|(id, _)| id.clone());
        }
        if candidates.len() == 1 {
            return Some(candidates[0].0.clone());
        }
        let in_scope = |ns: &str| candidates.iter().find(|(_, q)| q.strip_prefix(ns).is_some_and(|r| r.starts_with('.')));
        let mut scope = from_ns.map(|n| n.replace("::", "."));
        while let Some(ns) = scope {
            if let Some((id, _)) = in_scope(&ns) {
                return Some(id.clone());
            }
            scope = ns.rsplit_once('.').map(|(head, _)| head.to_owned());
        }
        candidates.iter().find(|(id, _)| !id.starts_with("ext:")).or(candidates.first()).map(|(id, _)| id.clone())
    }
}

fn simple_name(name: &str) -> String {
    let dotted = name.replace("::", ".");
    let last = dotted.rsplit('.').next().unwrap_or(&dotted);
    last.split(['<', '`']).next().unwrap_or(last).to_owned()
}

fn namespace_of(symbols: &HashMap<&str, &Symbol>, id: &str) -> Option<String> {
    let mut cur = *symbols.get(id)?;
    loop {
        if matches!(cur.kind, SymbolKind::Namespace | SymbolKind::Module) {
            return Some(cur.qualified_name.clone());
        }
        cur = symbols.get(cur.parent.as_deref()?)?;
    }
}

fn external_class(id: &str, name: &str, language: Language) -> Symbol {
    Symbol {
        id: id.to_owned(),
        kind: SymbolKind::Class,
        name: simple_name(name),
        qualified_name: name.to_owned(),
        parent: None,
        language,
        origins: Vec::new(),
        flavors: Vec::new(),
        signature: None,
        type_text: None,
        has_flow: false,
        external: true,
        doc: None,
    }
}

fn info(id: &str) -> SymbolInfo {
    SymbolInfo { symbol: id.to_owned(), complexity: 0, capabilities: Vec::new(), entry: None, test: false }
}
