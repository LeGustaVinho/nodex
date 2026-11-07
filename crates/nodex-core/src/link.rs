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
                    external: false,
                    ..external_symbol(&from, SymbolKind::Namespace, &imp.from_scope, file.language)
                });
            }
            edges.insert((from, to, StructEdgeKind::Imports));
        }
    }

    // 3. Type references.
    let index = TypeIndex::new(&symbols);
    let imports_by_file: HashMap<&str, Vec<&str>> = files
        .iter()
        .map(|f| (f.file.as_str(), f.imports.iter().map(|i| i.target.as_str()).collect()))
        .collect();
    for file in files {
        let imports = imports_by_file.get(file.file.as_str()).cloned().unwrap_or_default();
        for r in &file.type_refs {
            let from_ns = namespace_of(&symbols, &r.from);
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
                    let looks_like_interface = short.len() > 1
                        && short.starts_with('I')
                        && short.chars().nth(1).is_some_and(char::is_uppercase);
                    let kind = if looks_like_interface { SymbolKind::Interface } else { SymbolKind::Class };
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
    StructureGraph {
        symbols: all,
        edges: edges.into_iter().map(|(from, to, kind)| StructEdge { from, to, kind }).collect(),
    }
}

fn external_symbol(id: &str, kind: SymbolKind, name: &str, language: Language) -> Symbol {
    Symbol {
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

    /// `id` itself, or its `/index` module.
    fn resolve(&self, symbols: &BTreeMap<String, Symbol>, id: &str) -> Option<String> {
        if symbols.contains_key(id) {
            return Some(id.to_owned());
        }
        let index = format!("{id}/index");
        symbols.contains_key(&index).then_some(index)
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

/// Type lookup by simple name.
struct TypeIndex {
    by_name: HashMap<String, Vec<(String, String)>>, // simple name -> (id, qualified name)
}

impl TypeIndex {
    fn new(symbols: &BTreeMap<String, Symbol>) -> Self {
        let mut by_name: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for s in symbols.values().filter(|s| s.kind.is_type()) {
            let simple = s.id.rsplit('.').next().unwrap_or(&s.id);
            let simple = simple.split('`').next().unwrap_or(simple).to_owned();
            by_name.entry(simple).or_default().push((s.id.clone(), s.qualified_name.clone()));
        }
        Self { by_name }
    }

    fn resolve(&self, name: &str, from_ns: Option<&str>, imports: &[&str]) -> Option<String> {
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
        let in_ns = |ns: &str, q: &str| q.strip_prefix(ns).is_some_and(|rest| rest.starts_with('.'));
        if name.contains('.')
            && let Some((id, _)) = candidates.iter().find(|(_, q)| q.ends_with(name)) {
                return Some(id.clone());
            }
        if let Some(ns) = from_ns {
            // Same namespace, then enclosing namespaces.
            let mut scope = Some(ns);
            while let Some(s) = scope {
                if let Some((id, _)) = candidates.iter().find(|(_, q)| in_ns(s, q) && !q[s.len() + 1..].contains('.')) {
                    return Some(id.clone());
                }
                scope = s.rsplit_once('.').map(|(head, _)| head);
            }
        }
        for imp in imports {
            if let Some((id, _)) = candidates.iter().find(|(_, q)| in_ns(imp, q)) {
                return Some(id.clone());
            }
        }
        candidates.first().map(|(id, _)| id.clone())
    }
}
