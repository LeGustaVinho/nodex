//! Links per-file IR into one [`StructureGraph`]: merges partial declarations, resolves type
//! references by name, and turns imports into namespace edges.
//!
//! Resolution here is syntactic (names + imports). Semantic providers (milestone M3) replace it
//! with exact symbol resolution when available.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nodex_ir::{
    FileIr, Language, PackageRef, StructEdge, StructEdgeKind, StructureGraph, Symbol, SymbolKind, TypeRelation,
};

pub fn link(files: &[&FileIr], packages: &[(Language, PackageRef)]) -> StructureGraph {
    // 1. Merge symbols by id.
    let mut symbols: BTreeMap<String, Symbol> = BTreeMap::new();
    for file in files {
        for s in &file.symbols {
            match symbols.get_mut(&s.id) {
                Some(existing) => {
                    for o in &s.origins {
                        if !existing.origins.contains(o) {
                            existing.origins.push(o.clone());
                        }
                    }
                    for f in &s.flavors {
                        if !existing.flavors.contains(f) {
                            existing.flavors.push(f.clone());
                        }
                    }
                    if let Some(sites) = &s.syntax_sites {
                        let existing_sites = existing.syntax_sites.get_or_insert_with(Vec::new);
                        for site in sites {
                            if !existing_sites.contains(site) {
                                existing_sites.push(site.clone());
                            }
                        }
                    }
                    if let Some(contract) = &s.rust_contract {
                        let items = existing.rust_contract.get_or_insert_with(Vec::new);
                        for item in contract {
                            if !items.contains(item) {
                                items.push(item.clone());
                            }
                        }
                    }
                    existing.has_flow |= s.has_flow;
                    if existing.origins.len() > 1 && existing.kind.is_type() && !existing.flavors.iter().any(|f| f == "partial") {
                        existing.flavors.push("partial".into());
                    }
                }
                None => {
                    symbols.insert(s.id.clone(), s.clone());
                }
            }
        }
    }

    let renamed = merge_impl_blocks(&mut symbols);

    let mut edges: BTreeSet<(String, String, StructEdgeKind)> = BTreeSet::new();
    let mut external: BTreeMap<String, Symbol> = BTreeMap::new();

    // 2. Imports between namespaces/modules, and from modules to packages.
    let modules = ModuleIndex::new(&symbols);
    let declared: BTreeSet<&str> = packages.iter().map(|(_, p)| p.name.as_str()).collect();
    for file in files {
        for imp in &file.imports {
            let from = format!("ns:{}", imp.from_scope);
            let to = match imp.target.strip_prefix("pkg:") {
                Some(spec) => {
                    let name = package_name(spec);
                    match modules.resolve(&symbols, &format!("ns:{spec}")).filter(|_| !declared.contains(name.as_str())).or_else(|| {
                        if declared.contains(name.as_str()) { None } else { modules.alias(spec) }
                    }) {
                        Some(module) => module,
                        None => {
                            let id = format!("pkg:{name}");
                            external.entry(id.clone()).or_insert_with(|| external_symbol(&id, SymbolKind::Package, &name, file.language));
                            id
                        }
                    }
                }
                None => modules.resolve(&symbols, &format!("ns:{}", imp.target)).unwrap_or_else(|| format!("ns:{}", imp.target)),
            };
            if from == to {
                continue;
            }
            if !symbols.contains_key(&to) && !to.starts_with("pkg:") {
                external.entry(to.clone()).or_insert_with(|| external_symbol(&to, SymbolKind::Namespace, &imp.target, file.language));
            }
            if !symbols.contains_key(&from) {
                external.entry(from.clone()).or_insert_with(|| Symbol {
                    rust_contract: None,
                    external: false,
                    ..external_symbol(&from, SymbolKind::Namespace, &imp.from_scope, file.language)
                });
            }
            edges.insert((from, to, StructEdgeKind::Imports));
        }
    }

    // 3. Type references.
    let index = TypeIndex::new(&symbols);
    // Namespaces of the referring symbols, dotted (`app::orders` → `app.orders`), computed once.
    let mut namespaces: HashMap<String, Option<String>> = HashMap::new();
    for file in files {
        let imports: Vec<String> = file.imports.iter().map(|i| dotted(&i.target).into_owned()).collect();
        for r in &file.type_refs {
            let renamed_from = renamed.get(&r.from);
            let r = &nodex_ir::TypeRef { from: renamed_from.cloned().unwrap_or_else(|| r.from.clone()), name: r.name.clone(), relation: r.relation };
            let from_ns = namespaces
                .entry(r.from.clone())
                .or_insert_with(|| namespace_of(&symbols, &r.from).map(|n| dotted(&n).into_owned()));
            match index.resolve(&r.name, from_ns.as_deref(), &imports) {
                Some(target) if target != r.from => {
                    let kind = match r.relation {
                        TypeRelation::Base if symbols[&target].kind.is_contract() => StructEdgeKind::Implements,
                        TypeRelation::Base => StructEdgeKind::Inherits,
                        TypeRelation::References => StructEdgeKind::References,
                        TypeRelation::Instantiates => StructEdgeKind::Instantiates,
                    };
                    edges.insert((r.from.clone(), target, kind));
                }
                Some(_) => {}
                None if r.relation == TypeRelation::Base => {
                    // Unresolved base type: show it as an external type so inheritance stays visible.
                    let short = r.name.rsplit('.').next().unwrap_or(&r.name).to_owned();
                    // Rust only has traits as bases (`impl Display for Order`).
                    let looks_like_interface = file.language == Language::Rust
                        || short.len() > 1 && short.starts_with('I') && short.chars().nth(1).is_some_and(char::is_uppercase);
                    let kind = match (looks_like_interface, file.language) {
                        (true, Language::Rust) => SymbolKind::Trait,
                        (true, _) => SymbolKind::Interface,
                        _ => SymbolKind::Class,
                    };
                    let id = format!("ext:{}", r.name);
                    external.entry(id.clone()).or_insert_with(|| external_symbol(&id, kind, &r.name, file.language));
                    let edge = if looks_like_interface { StructEdgeKind::Implements } else { StructEdgeKind::Inherits };
                    edges.insert((r.from.clone(), id, edge));
                }
                None => {}
            }
        }
    }

    // 4. Containment.
    for s in symbols.values() {
        if let Some(p) = &s.parent {
            edges.insert((p.clone(), s.id.clone(), StructEdgeKind::Contains));
        }
    }

    // 5. Packages.
    for (language, p) in packages {
        let id = format!("pkg:{}", p.name);
        let mut sym = external_symbol(&id, SymbolKind::Package, &p.name, *language);
        sym.signature = p.version.as_ref().map(|v| format!("{} {}", p.name, v));
        external.entry(id).or_insert(sym);
    }

    let mut all: Vec<Symbol> = symbols.into_values().collect();
    all.extend(external.into_values());
    let revisions: BTreeMap<_, _> = files.iter()
        .filter(|f| f.symbols.iter().any(|s| s.syntax_sites.is_some() || s.rust_contract.is_some()))
        .filter_map(|f| Some((f.file.clone(), f.source_revision.clone()?)))
        .collect();
    StructureGraph {
        source_revisions: (!revisions.is_empty()).then_some(revisions),
        symbols: all,
        edges: edges.into_iter().map(|(from, to, kind)| StructEdge::new(from, to, kind)).collect(),
    }
}

/// Rust `impl` blocks written in another file than their type produce a placeholder type
/// (flavor `impl-only`). When exactly one real type of the same crate has that name, the
/// placeholder's members move to it. Returns placeholder id → real id.
fn merge_impl_blocks(symbols: &mut BTreeMap<String, Symbol>) -> HashMap<String, String> {
    let mut renamed = HashMap::new();
    let placeholders: Vec<(String, String)> = symbols
        .values()
        .filter(|s| s.language == Language::Rust && s.flavors.iter().any(|f| f == "impl-only"))
        .map(|s| (s.id.clone(), s.name.clone()))
        .collect();
    for (id, name) in placeholders {
        let krate = id.split("::").next().unwrap_or("");
        let candidates: Vec<&Symbol> = symbols
            .values()
            .filter(|s| {
                s.language == Language::Rust
                    && s.kind.is_type()
                    && s.name == name
                    && s.id != id
                    && !s.flavors.iter().any(|f| f == "impl-only")
                    && s.id.split("::").next() == Some(krate)
            })
            .collect();
        let [real] = candidates.as_slice() else { continue };
        let real_id = real.id.clone();
        let Some(placeholder) = symbols.remove(&id) else { continue };
        if let Some(target) = symbols.get_mut(&real_id) {
            for f in placeholder.flavors.iter().filter(|f| f.starts_with("impl:") || f.as_str() == "unsafe") {
                if !target.flavors.contains(f) {
                    target.flavors.push(f.clone());
                }
            }
        }
        for s in symbols.values_mut() {
            if s.parent.as_deref() == Some(id.as_str()) {
                s.parent = Some(real_id.clone());
            }
        }
        renamed.insert(id, real_id);
    }
    renamed
}

fn external_symbol(id: &str, kind: SymbolKind, name: &str, language: Language) -> Symbol {
    Symbol {
        rust_contract: None,
        syntax_sites: None,
        type_info: None,
        value_info: None,
        id: id.to_owned(),
        kind,
        name: name.rsplit('.').next().unwrap_or(name).to_owned(),
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

/// The namespace (or module) containing `id`, following parents.
fn namespace_of(symbols: &BTreeMap<String, Symbol>, id: &str) -> Option<String> {
    let mut cur = symbols.get(id)?;
    loop {
        if matches!(cur.kind, SymbolKind::Namespace | SymbolKind::Module) {
            return Some(cur.qualified_name.clone());
        }
        cur = symbols.get(cur.parent.as_ref()?)?;
    }
}

/// Module lookup for file-based languages (TypeScript): `ns:src/models` may name
/// `src/models/index`, and path aliases (`@/models/order`) end like a project module.
struct ModuleIndex {
    /// Module paths (without `ns:`), shortest first.
    paths: Vec<String>,
}

impl ModuleIndex {
    fn new(symbols: &BTreeMap<String, Symbol>) -> Self {
        let mut paths: Vec<String> = symbols
            .values()
            .filter(|s| s.kind == SymbolKind::Module)
            .filter_map(|s| s.id.strip_prefix("ns:").map(str::to_owned))
            .collect();
        paths.sort_by_key(|p| (p.len(), p.clone()));
        Self { paths }
    }

    /// `id` itself, its `/index` module (TypeScript), or the module that holds the imported
    /// item (Rust: `ns:app::orders::Order` → `ns:app::orders`).
    fn resolve(&self, symbols: &BTreeMap<String, Symbol>, id: &str) -> Option<String> {
        if symbols.contains_key(id) {
            return Some(id.to_owned());
        }
        let index = format!("{id}/index");
        if symbols.contains_key(&index) {
            return Some(index);
        }
        let mut cur = id;
        while let Some((head, _)) = cur.rsplit_once("::") {
            if symbols.get(head).is_some_and(|s| s.kind == SymbolKind::Module) {
                return Some(head.to_owned());
            }
            cur = head;
        }
        None
    }

    /// A non-relative specifier that is not a declared package: `@/x/y`, `~/x/y`, `#x/y`, or a
    /// `baseUrl`-relative `services/orders`. Resolved to the shortest module path ending with it.
    fn alias(&self, spec: &str) -> Option<String> {
        if self.paths.is_empty() {
            return None;
        }
        let tail = spec
            .strip_prefix("@/")
            .or_else(|| spec.strip_prefix("~/"))
            .or_else(|| spec.strip_prefix('#'))
            .unwrap_or(spec);
        if tail.is_empty() || !tail.contains('/') && !spec.starts_with(['@', '~', '#']) {
            // A bare single word (`react`, `fs`) is a package, not a path.
            return None;
        }
        for candidate in [tail.to_owned(), format!("{tail}/index")] {
            if let Some(p) = self.paths.iter().find(|p| *p == &candidate || p.ends_with(&format!("/{candidate}"))) {
                return Some(format!("ns:{p}"));
            }
        }
        None
    }
}

/// Node built-in modules, shown as `node:<name>` packages.
const NODE_BUILTINS: &[&str] = &[
    "assert", "async_hooks", "buffer", "child_process", "cluster", "crypto", "dgram", "dns", "events", "fs",
    "http", "http2", "https", "module", "net", "os", "path", "perf_hooks", "process", "querystring", "readline",
    "stream", "string_decoder", "timers", "tls", "tty", "url", "util", "v8", "vm", "worker_threads", "zlib",
];

/// The package an import specifier belongs to: `@scope/pkg/sub` → `@scope/pkg`, `lodash/fp` →
/// `lodash`, `fs/promises` → `node:fs`.
pub fn package_name(spec: &str) -> String {
    if let Some(rest) = spec.strip_prefix("node:") {
        return format!("node:{}", rest.split('/').next().unwrap_or(rest));
    }
    let mut parts = spec.split('/');
    let first = parts.next().unwrap_or(spec);
    if first.starts_with('@') {
        return match parts.next() {
            Some(second) => format!("{first}/{second}"),
            None => first.to_owned(),
        };
    }
    if NODE_BUILTINS.contains(&first) {
        return format!("node:{first}");
    }
    first.to_owned()
}

/// Rust paths (`app::orders::Order`) compare like dotted names.
fn dotted(name: &str) -> std::borrow::Cow<'_, str> {
    if name.contains("::") { name.replace("::", ".").into() } else { name.into() }
}

/// Type lookup by simple name.
struct TypeIndex {
    by_name: HashMap<String, Vec<(String, String)>>, // simple name -> (id, qualified name)
    /// Simple name -> namespace directly holding the type -> its first candidate. A common
    /// name (`Order`, `Program`) can have thousands of candidates in a large project.
    by_parent: HashMap<String, HashMap<String, usize>>,
    /// Simple name -> any namespace enclosing the type -> its first candidate (for imports).
    by_ancestor: HashMap<String, HashMap<String, usize>>,
}

impl TypeIndex {
    fn new(symbols: &BTreeMap<String, Symbol>) -> Self {
        let mut by_name: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for s in symbols.values().filter(|s| s.kind.is_type()) {
            let dotted = dotted(&s.id);
            let simple = dotted.rsplit('.').next().unwrap_or(&dotted);
            let simple = simple.split('`').next().unwrap_or(simple).to_owned();
            by_name.entry(simple).or_default().push((s.id.clone(), self::dotted(&s.qualified_name).into_owned()));
        }
        let mut by_parent: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut by_ancestor: HashMap<String, HashMap<String, usize>> = HashMap::new();
        for (simple, candidates) in by_name.iter().filter(|(_, c)| c.len() > 1) {
            let parents = by_parent.entry(simple.clone()).or_default();
            let ancestors = by_ancestor.entry(simple.clone()).or_default();
            for (i, (_, q)) in candidates.iter().enumerate() {
                if let Some((parent, _)) = q.rsplit_once('.') {
                    parents.entry(parent.to_owned()).or_insert(i);
                }
                let mut scope = q.as_str();
                while let Some((head, _)) = scope.rsplit_once('.') {
                    ancestors.entry(head.to_owned()).or_insert(i);
                    scope = head;
                }
            }
        }
        Self { by_name, by_parent, by_ancestor }
    }

    /// `from_ns` and `imports` are already dotted.
    fn resolve(&self, name: &str, from_ns: Option<&str>, imports: &[String]) -> Option<String> {
        let name = &*dotted(name);
        let simple = name.rsplit('.').next().unwrap_or(name);
        let candidates = self.by_name.get(simple)?;
        if candidates.len() == 1 {
            let (id, q) = &candidates[0];
            // A qualified reference must match its qualification.
            if name.contains('.') && !q.ends_with(name) {
                return None;
            }
            return Some(id.clone());
        }
        if name.contains('.')
            && let Some((id, _)) = candidates.iter().find(|(_, q)| q.ends_with(name)) {
                return Some(id.clone());
            }
        if let Some(ns) = from_ns {
            // Same namespace, then enclosing namespaces: a candidate directly inside the scope.
            let parents = self.by_parent.get(simple);
            let mut scope = Some(ns);
            while let Some(s) = scope {
                if let Some(&i) = parents.and_then(|p| p.get(s)) {
                    return Some(candidates[i].0.clone());
                }
                scope = s.rsplit_once('.').map(|(head, _)| head);
            }
        }
        let ancestors = self.by_ancestor.get(simple);
        for imp in imports {
            if let Some(&i) = ancestors.and_then(|a| a.get(imp.as_str())) {
                return Some(candidates[i].0.clone());
            }
        }
        candidates.first().map(|(id, _)| id.clone())
    }
}
