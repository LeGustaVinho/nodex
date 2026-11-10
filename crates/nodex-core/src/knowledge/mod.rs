//! Project-wide knowledge: links the per-file [`FileKnowledge`] facts and what flow graphs show
//! (complexity, throws) into knowledge edges and a [`ProjectKnowledge`] summary.
//!
//! The derived passes were ported from CodeGraph (`crates/semantic/src/global.rs`). Structural
//! relations (calls, inheritance) are not guessed here: they come from the linker and from the
//! semantic providers.
//!
//! [`FileKnowledge`]: nodex_ir::knowledge::FileKnowledge

pub mod flows;
pub mod global;
pub mod routes;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use nodex_ir::knowledge::{
    AuthInfo, EntryKind, EntryPoint, ModelInfo, ProjectKnowledge, ResolvedRoute, RouteFact, RoutePrefix, StateMachineFact, SymbolInfo,
};
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

/// A call reported by a semantic provider: where it is, who makes it, what it reaches.
pub struct RawCallSite {
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub owner: SymbolId,
    pub target: SymbolId,
}

/// Derives knowledge from the linked `graph` and the files' facts and flows. `package_roots` are
/// the project-relative folders of the manifests (`backend-nest`, `""` for the root): project-wide
/// route prefixes apply within one package.
pub fn build(graph: &StructureGraph, files: &[&FileIr], constructs: &Constructs, calls: &[RawCallSite], package_roots: &[String]) -> Knowledge {
    let types = TypeLookup::new(graph);
    let symbols: HashMap<&str, &Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut children: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for s in &graph.symbols {
        if let Some(p) = &s.parent {
            children.entry(p.as_str()).or_default().push(s);
        }
    }
    let mut out = Knowledge::default();
    let mut externals: BTreeMap<String, Symbol> = BTreeMap::new();
    let mut edges: BTreeMap<(String, String, StructEdgeKind), StructEdge> = BTreeMap::new();
    let mut infos: BTreeMap<SymbolId, SymbolInfo> = BTreeMap::new();
    let mut caps: BTreeMap<SymbolId, BTreeSet<String>> = BTreeMap::new();
    let add_cap = |caps: &mut BTreeMap<SymbolId, BTreeSet<String>>, id: &str, tag: &str| {
        caps.entry(id.to_owned()).or_default().insert(tag.to_owned());
    };
    // Exception types caught around each statement: file → (start, end, types).
    let mut protected: HashMap<&str, Vec<(u32, u32, Vec<String>)>> = HashMap::new();
    let mut handlers = Vec::new();
    let mut state_machines: Vec<StateMachineFact> = Vec::new();
    // Every throw: (function, exception type, types caught around the throw in that function).
    let mut throw_sites: Vec<(SymbolId, SymbolId, Vec<String>)> = Vec::new();

    for file in files {
        // Flows: complexity, throws and catch regions.
        for flow in &file.flows {
            let entry = infos.entry(flow.symbol.clone()).or_insert_with(|| info(&flow.symbol));
            entry.complexity = flows::complexity(flow);
            let regions = flows::catch_regions(flow);
            for (node_id, types) in &regions {
                if let Some(node) = flow.node(node_id) {
                    protected.entry(file.file.as_str()).or_default().push((node.span.start_byte, node.span.end_byte, types.clone()));
                }
            }
            let thrown = flows::throws(flow);
            if !thrown.is_empty() {
                add_cap(&mut caps, &flow.symbol, "THROWS");
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
                throw_sites.push((flow.symbol.clone(), target.clone(), regions.get(&t.node).cloned().unwrap_or_default()));
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
            set_entry(entry, s.entry.clone());
            entry.test |= s.test;
            if let Some(model) = &s.data_model {
                out.project.models.push(ModelInfo { symbol: s.symbol.clone(), model: model.clone() });
            }
            for c in &s.capabilities {
                add_cap(&mut caps, &s.symbol, c);
            }
        }
        for c in &k.config {
            add_cap(&mut caps, &c.owner, "CONFIG_READ");
        }
        for h in &k.http_calls {
            add_cap(&mut caps, &h.owner, "NETWORK_IO");
        }
        for m in &k.mutations {
            let tag = match m.kind.as_str() {
                "database" => "DB_WRITE",
                "cache" => "CACHE",
                "storage" => "LOCAL_STORAGE",
                _ => "STATE_WRITE",
            };
            add_cap(&mut caps, &m.owner, tag);
        }
        for a in &k.auth {
            let entry = infos.entry(a.owner.clone()).or_insert_with(|| info(&a.owner));
            let auth = entry.auth.get_or_insert_with(AuthInfo::default);
            for r in &a.roles {
                if !auth.roles.contains(r) {
                    auth.roles.push(r.clone());
                }
            }
            for p in &a.policies {
                if !auth.policies.contains(p) {
                    auth.policies.push(p.clone());
                }
            }
            auth.anonymous |= a.anonymous;
            if !a.anonymous {
                add_cap(&mut caps, &a.owner, "AUTH_REQUIRED");
            }
        }
        state_machines.extend(k.state_machines.iter().cloned());
        handlers.extend(k.handlers.iter().cloned());
        out.project.http_calls.extend(k.http_calls.iter().cloned());
        out.project.di.extend(k.di.iter().cloned());
        out.project.config.extend(k.config.iter().cloned());
    }

    // Entry points shown by flavors: engine messages, top-level programs.
    for s in &graph.symbols {
        let kind = if s.flavors.iter().any(|f| f == "unity:message") {
            Some((EntryKind::Framework, "Unity message"))
        } else if s.flavors.iter().any(|f| f == "react:component") {
            Some((EntryKind::Framework, "React component"))
        } else if s.flavors.iter().any(|f| f == "top-level") && s.has_flow && s.language == Language::Csharp {
            Some((EntryKind::Main, "program entry (top-level statements)"))
        } else {
            None
        };
        if let Some((kind, label)) = kind {
            let entry = infos.entry(s.id.clone()).or_insert_with(|| info(&s.id));
            set_entry(entry, Some(EntryPoint { kind, label: label.into() }));
        }
    }

    out.project.routes = resolve_routes(files, &symbols, package_roots);
    for r in &out.project.routes {
        let entry = infos.entry(r.handler.clone()).or_insert_with(|| info(&r.handler));
        let point = if r.method == "PAGE" {
            EntryPoint { kind: EntryKind::Ui, label: format!("page {}", r.path) }
        } else {
            EntryPoint { kind: EntryKind::Route, label: format!("{} {}", r.method, r.path) }
        };
        set_entry(entry, Some(point));
    }

    // A class of test cases is a test too (test edges by naming convention start from it).
    let test_types: Vec<String> = infos
        .iter()
        .filter(|(id, i)| i.test && symbols.get(id.as_str()).is_some_and(|s| !s.kind.is_type()))
        .filter_map(|(id, _)| symbols.get(id.as_str()).and_then(|s| s.parent.clone()))
        .filter(|p| symbols.get(p.as_str()).is_some_and(|s| s.kind.is_type()))
        .collect();
    for t in test_types {
        infos.entry(t.clone()).or_insert_with(|| info(&t)).test = true;
    }

    // Project-wide relations.
    let throws: Vec<StructEdge> = edges.values().cloned().collect();
    let mut all_edges: Vec<StructEdge> = graph.edges.clone();
    all_edges.extend(throws.iter().cloned());
    let base = global::Ctx { symbols: &symbols, children: &children, types: &types, edges: &all_edges };
    let di = global::di_edges(&base, files);
    let injections = global::injection_edges(&base, files, &infos);
    let dispatch = global::di_dispatch(&base, &di);
    let (models, by_property) = global::models(&base, files);
    let mutates = global::mutation_edges(files, &by_property);
    out.project.models.extend(models);
    all_edges.extend(dispatch.iter().cloned());
    let ctx = global::Ctx { symbols: &symbols, children: &children, types: &types, edges: &all_edges };
    let tests = global::test_edges(&ctx, &infos);
    let http = global::http_edges(files, &out.project.routes);
    for t in &tests {
        if let Some(i) = infos.get_mut(&t.from) {
            i.test = true;
        }
    }

    // Call sites with the exception types caught around them; DI dispatch adds the implementation.
    let catches_at = |file: &str, start: u32, end: u32| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (s, e, types) in protected.get(file).into_iter().flatten() {
            if *s <= start && end <= *e {
                for t in types {
                    if !out.contains(t) {
                        out.push(t.clone());
                    }
                }
            }
        }
        out
    };
    let mut dispatch_from: HashMap<&str, Vec<&StructEdge>> = HashMap::new();
    for d in &dispatch {
        dispatch_from.entry(d.from.as_str()).or_default().push(d);
    }
    let mut sites: Vec<global::CallSite> = Vec::new();
    for c in calls {
        let catches = catches_at(&c.file, c.start, c.end);
        for d in dispatch_from.get(c.owner.as_str()).into_iter().flatten() {
            let reaches = symbols.get(d.to.as_str()).zip(symbols.get(c.target.as_str())).is_some_and(|(imp, iface)| imp.name == iface.name);
            if reaches {
                sites.push(global::CallSite { owner: c.owner.clone(), target: d.to.clone(), catches: catches.clone() });
            }
        }
        sites.push(global::CallSite { owner: c.owner.clone(), target: c.target.clone(), catches });
    }
    let inferred: HashSet<(String, String)> = dispatch.iter().map(|d| (d.from.clone(), d.to.clone())).collect();
    // Exceptions every throw site of a function catches itself never reach its callers.
    let lineages = global::Lineage::new(ctx.edges);
    let mut escaping: HashSet<(&str, &str)> = HashSet::new();
    let mut thrown: HashSet<(&str, &str)> = HashSet::new();
    for (from, ty, catches) in &throw_sites {
        thrown.insert((from, ty));
        if !lineages.handles(&ctx, ty, catches) {
            escaping.insert((from, ty));
        }
    }
    let caught_locally: HashSet<(String, String)> =
        thrown.difference(&escaping).map(|(f, t)| ((*f).to_owned(), (*t).to_owned())).collect();
    let caught = global::caught_by_edges(&ctx, &sites, &handlers, &infos, &inferred, &caught_locally);

    for e in di.into_iter().chain(injections).chain(dispatch).chain(mutates).chain(tests).chain(http).chain(caught) {
        edges.entry((e.from.clone(), e.to.clone(), e.kind)).or_insert(e);
    }

    for (id, info) in infos.iter_mut() {
        if let Some(sym) = symbols.get(id.as_str())
            && sym.flavors.iter().any(|f| f == "async")
        {
            add_cap(&mut caps, id, "ASYNC");
        }
        info.capabilities = caps.remove(id).map(|c| c.into_iter().collect()).unwrap_or_default();
    }
    // Capabilities of symbols without a flow (rare: facts on a field initializer).
    for (id, c) in caps {
        let mut i = info(&id);
        i.capabilities = c.into_iter().collect();
        infos.insert(id, i);
    }

    out.project.symbols = infos.into_values().filter(|i| symbols.contains_key(i.symbol.as_str())).collect();
    out.project.state_machines = merge_machines(state_machines);
    out.project.models.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    out.project.models.dedup_by(|a, b| a.symbol == b.symbol);
    out.edges = edges.into_values().collect();
    out.externals = externals.into_values().filter(|s| !symbols.contains_key(s.id.as_str())).collect();
    out
}

/// The package (manifest folder) a file belongs to: the deepest root that contains it.
fn package_of<'r>(file: &str, roots: &'r [String]) -> &'r str {
    roots
        .iter()
        .filter(|r| r.is_empty() || file.strip_prefix(r.as_str()).is_some_and(|rest| rest.starts_with('/')))
        .max_by_key(|r| r.len())
        .map(String::as_str)
        .unwrap_or("")
}

/// Resolves the handlers named in another file, applies project-wide prefixes and normalizes
/// every route. A `global` prefix applies to the routes of its framework in the same package; a
/// package without one (a library of controllers) takes the framework's prefix when the project
/// has only one.
pub fn resolve_routes(files: &[&FileIr], symbols: &HashMap<&str, &Symbol>, package_roots: &[String]) -> Vec<ResolvedRoute> {
    let mut global: HashMap<(&str, &str), &str> = HashMap::new();
    let mut by_framework: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for file in files {
        let package = package_of(&file.file, package_roots);
        for p in file.knowledge.prefixes.iter().filter(|p| p.kind == "global") {
            global.insert((p.framework.as_str(), package), p.prefix.as_str());
            by_framework.entry(p.framework.as_str()).or_default().insert(p.prefix.as_str());
        }
    }
    let prefix_of = |framework: &str, package: &str| -> Option<&str> {
        global.get(&(framework, package)).copied().or_else(|| {
            let all = by_framework.get(framework)?;
            (all.len() == 1).then(|| *all.iter().next().unwrap())
        })
    };
    let mut functions: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for s in symbols.values().filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method) && !s.external) {
        functions.entry(s.name.as_str()).or_default().push(s);
    }
    let mounts = Mounts::new(files, package_roots);
    let mut out: Vec<ResolvedRoute> = Vec::new();
    for file in files {
        let package = package_of(&file.file, package_roots);
        for r in &file.knowledge.routes {
            let handler = resolve_handler(r, file.language, &functions, symbols).unwrap_or_else(|| r.handler.clone());
            let handler_name = symbols.get(handler.as_str()).map(|s| s.name.clone()).unwrap_or_default();
            // Mounted routers take every prefix they are mounted under (one route per mount).
            let mounted = if r.method == "PAGE" { Vec::new() } else { mounts.prefixes(&file.file, r.router.as_deref(), &handler_name, 0) };
            let local: Vec<String> = if mounted.is_empty() { vec![String::new()] } else { mounted };
            for mount in local {
                let declared = routes::join_route(&mount, &r.path);
                let path = match prefix_of(r.framework.as_str(), package) {
                    Some(prefix) if r.method != "PAGE" => routes::join_route(prefix, &declared),
                    _ => declared,
                };
                out.push(ResolvedRoute {
                    handler: handler.clone(),
                    method: r.method.clone(),
                    normalized: routes::normalize_route(&path),
                    path,
                    framework: r.framework.clone(),
                    span: r.span.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| (&a.normalized, &a.method, &a.handler).cmp(&(&b.normalized, &b.method, &b.handler)));
    out.dedup_by(|a, b| a.normalized == b.normalized && a.method == b.method && a.handler == b.handler);
    out
}

/// Prefixes of routers mounted elsewhere: `app.use('/api', ordersRouter)` (Express, Koa),
/// `.nest("/api", order_routes())` (axum), `web::scope("/api").service(h)` (actix),
/// `.mount("/api", routes![a, b])` (Rocket).
///
/// A mount `target` names what it mounts:
/// - `module:<path>`: the routers of a project file (an imported router, `require('./x')`);
/// - `fn:<name>`: the routes built by a function of that name in the same package;
/// - `handler:<name>`: one route handler of that name in the same package;
/// - anything else: a router variable of the same file.
///
/// A mount declared on a router that is itself mounted takes that router's prefixes too.
struct Mounts<'a> {
    entries: Vec<(&'a str, &'a str, &'a RoutePrefix)>,
    roots: &'a [String],
}

impl<'a> Mounts<'a> {
    fn new(files: &[&'a FileIr], roots: &'a [String]) -> Self {
        let mut entries = Vec::new();
        for f in files {
            for p in f.knowledge.prefixes.iter().filter(|p| p.kind == "mount") {
                entries.push((f.file.as_str(), package_of(&f.file, roots), p));
            }
        }
        Self { entries, roots }
    }

    /// Every prefix a route of `file` declared on `router` (handler `handler`) is served under;
    /// empty when nothing mounts it.
    fn prefixes(&self, file: &str, router: Option<&str>, handler: &str, depth: usize) -> Vec<String> {
        if depth > 6 {
            return Vec::new();
        }
        let package = package_of(file, self.roots);
        let stem = strip_ext(file);
        let mut out = Vec::new();
        for (mount_file, mount_package, m) in &self.entries {
            let hit = if let Some(module) = m.target.strip_prefix("module:") {
                module == stem || format!("{module}/index") == stem
            } else if let Some(f) = m.target.strip_prefix("fn:") {
                *mount_package == package && router.is_some_and(|r| r == m.target || r.strip_prefix("fn:") == Some(f))
            } else if let Some(h) = m.target.strip_prefix("handler:") {
                *mount_package == package && !handler.is_empty() && h == handler
            } else {
                *mount_file == file && router == Some(m.target.as_str())
            };
            if !hit {
                continue;
            }
            // The router that declares the mount may be mounted itself.
            let outer = self.prefixes(mount_file, m.router.as_deref(), "", depth + 1);
            if outer.is_empty() {
                out.push(m.prefix.clone());
            } else {
                out.extend(outer.into_iter().map(|o| routes::join_route(&o, &m.prefix)));
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// `src/routes/orders.ts` → `src/routes/orders`.
fn strip_ext(file: &str) -> &str {
    let name_at = file.rfind('/').map(|i| i + 1).unwrap_or(0);
    match file[name_at..].find('.') {
        Some(dot) => &file[..name_at + dot],
        None => file,
    }
}

/// One machine per owner and name (a partial class configures it in several files).
fn merge_machines(machines: Vec<StateMachineFact>) -> Vec<StateMachineFact> {
    let mut by_key: BTreeMap<(String, String, String), StateMachineFact> = BTreeMap::new();
    for m in machines {
        let key = (m.owner.clone(), m.name.clone(), m.framework.clone());
        match by_key.get_mut(&key) {
            None => {
                by_key.insert(key, m);
            }
            Some(acc) => {
                for s in m.states {
                    if !acc.states.contains(&s) {
                        acc.states.push(s);
                    }
                }
                for t in m.transitions {
                    if !acc.transitions.contains(&t) {
                        acc.transitions.push(t);
                    }
                }
                for t in m.triggers {
                    if !acc.triggers.contains(&t) {
                        acc.triggers.push(t);
                    }
                }
                acc.initial = acc.initial.take().or(m.initial);
            }
        }
    }
    by_key.into_values().collect()
}

/// The function a route names (`ordersController.list`, `Handlers.Get`, `handlers::list`): with a
/// qualifier, the only function of that name whose owner matches it (a class, an instance named
/// after it, or a module); without one, the only function of that name in the language. `None`
/// keeps the frontend's guess.
fn resolve_handler(r: &RouteFact, language: Language, functions: &HashMap<&str, Vec<&Symbol>>, symbols: &HashMap<&str, &Symbol>) -> Option<SymbolId> {
    let written = r.handler_ref.as_deref()?.replace("::", ".");
    let mut parts: Vec<&str> = written.split('.').filter(|p| !p.is_empty() && *p != "this").collect();
    let name = parts.pop()?;
    let candidates: Vec<&Symbol> = functions.get(name)?.iter().copied().filter(|s| s.language == language).collect();
    let owner_matches = |s: &Symbol, q: &str| {
        let Some(parent) = s.parent.as_deref().and_then(|p| symbols.get(p)) else { return false };
        let tail = parent.qualified_name.rsplit(['/', '.', ':']).next().unwrap_or("");
        let file_stem = parent.qualified_name.rsplit('/').next().unwrap_or("").split('.').next().unwrap_or("");
        [parent.name.as_str(), tail, file_stem].iter().any(|n| !n.is_empty() && n.eq_ignore_ascii_case(q))
    };
    // A qualifier no owner matches (an object literal, a package import) names something else.
    let pick: Vec<&Symbol> = match parts.last() {
        Some(q) => candidates.iter().copied().filter(|s| owner_matches(s, q)).collect(),
        None => candidates,
    };
    match pick.as_slice() {
        [one] => Some(one.id.clone()),
        // Several: keep the frontend's choice when it is one of them.
        _ => None,
    }
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

/// Keeps the strongest reason a symbol is an entry point: a program entry beats a route, which
/// beats a command, a message consumer, a job, an event handler, a UI handler, a test, an
/// export, a framework hook or a server start.
fn set_entry(info: &mut SymbolInfo, entry: Option<EntryPoint>) {
    let Some(entry) = entry else { return };
    let rank = |k: EntryKind| match k {
        EntryKind::Main => 0,
        EntryKind::Route => 1,
        EntryKind::Command => 2,
        EntryKind::Message => 3,
        EntryKind::Job => 4,
        EntryKind::EventHandler => 5,
        EntryKind::Ui => 6,
        EntryKind::Test => 7,
        EntryKind::Export => 8,
        EntryKind::Framework => 9,
        EntryKind::ServerStart => 10,
    };
    if info.entry.as_ref().is_none_or(|old| rank(entry.kind) < rank(old.kind)) {
        info.entry = Some(entry);
    }
}

pub(crate) fn simple_name(name: &str) -> String {
    let dotted = name.replace("::", ".");
    let last = dotted.rsplit('.').next().unwrap_or(&dotted);
    last.split(['<', '`']).next().unwrap_or(last).to_owned()
}

pub(crate) fn namespace_of(symbols: &HashMap<&str, &Symbol>, id: &str) -> Option<String> {
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
        syntax_sites: None,
        type_info: None,
        value_info: None,
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
    SymbolInfo { symbol: id.to_owned(), complexity: 0, capabilities: Vec::new(), entry: None, test: false, auth: None }
}
