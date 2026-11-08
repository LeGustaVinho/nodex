//! Project-wide knowledge relations, ported from CodeGraph (`crates/semantic/src/global.rs` and
//! `models.rs`): dependency injection, data models and their writes, tests, and where thrown
//! exceptions are caught.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::rc::Rc;

use nodex_ir::knowledge::{Confidence, DataModel, EntityFact, EntryKind, GlobalHandlerFact, ModelField, ModelInfo, SymbolInfo};
use nodex_ir::{FileIr, StructEdge, StructEdgeKind, Symbol, SymbolId, SymbolKind};

use super::{TypeLookup, namespace_of, simple_name};

/// Read-only views over the linked graph shared by the passes.
pub struct Ctx<'a> {
    pub symbols: &'a HashMap<&'a str, &'a Symbol>,
    pub children: &'a HashMap<&'a str, Vec<&'a Symbol>>,
    pub types: &'a TypeLookup,
    pub edges: &'a [StructEdge],
}

impl Ctx<'_> {
    fn resolve(&self, name: &str, from: &str) -> Option<SymbolId> {
        self.types.resolve(name, namespace_of(self.symbols, from).as_deref())
    }

    fn name(&self, id: &str) -> String {
        self.symbols.get(id).map(|s| s.name.clone()).unwrap_or_else(|| id.trim_start_matches("ext:").to_owned())
    }
}

/// `services.AddScoped<IOrderService, OrderService>()`: the service type is provided by the
/// implementation. Exact: the registration says so.
pub fn di_edges(ctx: &Ctx, files: &[&FileIr]) -> Vec<StructEdge> {
    let mut out = Vec::new();
    // Injection points by token: `@Inject("TOKEN") gateway: PaymentGateway`.
    let mut injected: HashMap<&str, Vec<&nodex_ir::knowledge::DiFact>> = HashMap::new();
    for d in files.iter().flat_map(|f| f.knowledge.di.iter()).filter(|d| d.lifetime == "inject") {
        if let Some(t) = &d.token {
            injected.entry(t.as_str()).or_default().push(d);
        }
    }
    for file in files {
        for d in file.knowledge.di.iter().filter(|d| d.lifetime != "inject") {
            let Some(implementation) = &d.implementation else { continue };
            // A token registration provides the types injected under that token.
            if let Some(token) = &d.token {
                for point in injected.get(token.as_str()).into_iter().flatten() {
                    let (Some(service), Some(imp)) = (ctx.resolve(&point.service, &point.owner), ctx.resolve(implementation, &d.owner)) else { continue };
                    if service != imp {
                        out.push(
                            StructEdge::new(service, imp, StructEdgeKind::ResolvesDi)
                                .with_evidence(format!("provided for token \"{token}\" ({}) in {}", d.framework, ctx.name(&d.owner)))
                                .with_site(d.span.clone()),
                        );
                    }
                }
                continue;
            }
            let (Some(service), Some(imp)) = (ctx.resolve(&d.service, &d.owner), ctx.resolve(implementation, &d.owner)) else { continue };
            if service == imp {
                continue;
            }
            out.push(
                StructEdge::new(service, imp, StructEdgeKind::ResolvesDi)
                    .with_evidence(format!("registered as {} ({}) in {}", d.lifetime, d.framework, ctx.name(&d.owner)))
                    .with_site(d.span.clone()),
            );
        }
    }
    out
}

/// Constructor injection: a type whose constructor takes a service depends on it
/// (`OrderService(IOrderRepository repo)`, Nest `constructor(private orders: OrdersService)`).
/// Exact when the service is registered for injection; inferred when only the consumer is
/// known to be built by the container (a controller, a Nest provider).
pub fn injection_edges(ctx: &Ctx, files: &[&FileIr], infos: &BTreeMap<SymbolId, SymbolInfo>) -> Vec<StructEdge> {
    let mut registered: HashSet<SymbolId> = HashSet::new();
    for d in files.iter().flat_map(|f| f.knowledge.di.iter()).filter(|d| d.lifetime != "inject") {
        for name in std::iter::once(&d.service).chain(d.implementation.iter()) {
            if let Some(id) = ctx.resolve(name, &d.owner) {
                registered.insert(id);
            }
        }
    }
    let owning_type = |id: &str| -> Option<SymbolId> {
        let mut cur = *ctx.symbols.get(id)?;
        loop {
            if cur.kind.is_type() {
                return Some(cur.id.clone());
            }
            cur = ctx.symbols.get(cur.parent.as_deref()?)?;
        }
    };
    // A type with a route handler is a controller: the framework constructs it.
    let routed: HashSet<SymbolId> = infos
        .values()
        .filter(|i| i.entry.as_ref().is_some_and(|e| e.kind == EntryKind::Route))
        .filter_map(|i| owning_type(&i.symbol))
        .collect();
    let mut out: BTreeMap<(SymbolId, SymbolId), StructEdge> = BTreeMap::new();
    for file in files {
        for d in file.knowledge.di.iter().filter(|d| d.lifetime == "inject" && d.token.is_none()) {
            let Some(consumer) = owning_type(&d.owner) else { continue };
            let Some(service) = ctx.resolve(&d.service, &d.owner) else { continue };
            if service == consumer || ctx.symbols.get(service.as_str()).is_none_or(|s| s.external || !s.kind.is_type()) {
                continue;
            }
            // Exact when the container both builds the consumer and provides the service.
            let built = registered.contains(&consumer) || routed.contains(&consumer) || d.framework == "nestjs";
            let provided = registered.contains(&service);
            let confidence = match (built, provided) {
                (true, true) => Confidence::Exact,
                (false, false) => continue,
                _ => Confidence::Inferred,
            };
            out.entry((consumer.clone(), service.clone())).or_insert_with(|| {
                StructEdge::new(consumer.clone(), service.clone(), StructEdgeKind::DependsOn)
                    .with_confidence(confidence)
                    .with_evidence(format!("{} is injected into {} through its constructor", ctx.name(&service), ctx.name(&consumer)))
                    .with_site(d.span.clone())
            });
        }
    }
    out.into_values().collect()
}

/// A call to a method of an interface whose only DI implementation is known also reaches that
/// implementation's method. Inferred: the compiler only sees the interface.
pub fn di_dispatch(ctx: &Ctx, di: &[StructEdge]) -> Vec<StructEdge> {
    // Distinct implementations: one pair registered twice is still one implementation.
    let mut impls: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in di {
        let list = impls.entry(e.from.as_str()).or_default();
        if !list.contains(&e.to.as_str()) {
            list.push(e.to.as_str());
        }
    }
    let mut out = Vec::new();
    for call in ctx.edges.iter().filter(|e| e.kind == StructEdgeKind::Calls) {
        let Some(target) = ctx.symbols.get(call.to.as_str()) else { continue };
        let Some(owner) = target.parent.as_deref() else { continue };
        let Some([imp]) = impls.get(owner).map(Vec::as_slice) else { continue };
        let Some(member) = ctx.children.get(imp).and_then(|kids| kids.iter().find(|k| k.name == target.name && k.kind == target.kind)) else { continue };
        out.push(
            StructEdge::new(call.from.clone(), member.id.clone(), StructEdgeKind::Calls)
                .with_confidence(Confidence::Inferred)
                .with_evidence(format!("{} is provided by {} through dependency injection", ctx.name(owner), ctx.name(imp))),
        );
    }
    out
}

/// Data models named by entity facts, with fields read from the entity type's members.
/// Returns the models and, per context property (`Orders`), the entity it exposes.
pub fn models(ctx: &Ctx, files: &[&FileIr]) -> (Vec<ModelInfo>, HashMap<String, SymbolId>) {
    let entities: Vec<&EntityFact> = files.iter().flat_map(|f| f.knowledge.entities.iter()).collect();
    let mut by_entity: BTreeMap<SymbolId, &EntityFact> = BTreeMap::new();
    let mut by_property = HashMap::new();
    for e in &entities {
        let Some(id) = ctx.resolve(&e.entity, &e.owner) else { continue };
        if ctx.symbols.get(id.as_str()).is_some_and(|s| s.external) {
            continue;
        }
        if let Some(p) = &e.property {
            by_property.insert(p.clone(), id.clone());
        }
        by_entity.entry(id).or_insert(e);
    }
    let names: HashSet<String> = by_entity.keys().map(|id| ctx.name(id)).collect();
    let mut out = Vec::new();
    for (id, fact) in &by_entity {
        let entity_name = ctx.name(id);
        let mut fields = Vec::new();
        for m in ctx.children.get(id.as_str()).into_iter().flatten() {
            if !matches!(m.kind, SymbolKind::Property | SymbolKind::Field) || m.flavors.iter().any(|f| f == "static" || f == "const") {
                continue;
            }
            let ty = m.type_text.clone();
            let lower = m.name.to_ascii_lowercase();
            let entity_lower = entity_name.to_ascii_lowercase();
            let key = m.flavors.iter().any(|f| f == "attr:Key" || f == "attr:PrimaryGeneratedColumn" || f == "attr:PrimaryColumn")
                || lower == "id"
                || lower == format!("{entity_lower}id")
                || lower == format!("{entity_lower}_id");
            // `CustomerId`, `customer_id`: a foreign key to `Customer`.
            let fk_stem = (!key).then(|| m.name.strip_suffix("_id").or_else(|| m.name.strip_suffix("Id"))).flatten().filter(|s| !s.is_empty());
            let element = ty.as_deref().map(element_type).unwrap_or_default();
            let target = if names.contains(&element) && element != entity_name {
                Some(element.clone())
            } else {
                fk_stem.and_then(|stem| names.iter().find(|n| n.eq_ignore_ascii_case(&stem.replace('_', ""))).cloned())
            };
            fields.push(ModelField {
                name: m.name.clone(),
                ty,
                primary_key: key,
                foreign_key: fk_stem.is_some(),
                target,
            });
        }
        out.push(ModelInfo { symbol: id.clone(), model: DataModel { framework: fact.framework.clone(), table: fact.table.clone(), fields } });
    }
    (out, by_property)
}

/// `List<OrderItem>` → `OrderItem`, `Customer?` → `Customer`, `Order[]` → `Order`.
fn element_type(ty: &str) -> String {
    let t = ty.trim().trim_end_matches('?').trim_end_matches("[]");
    let inner = match (t.find('<'), t.rfind('>')) {
        (Some(a), Some(b)) if b > a => &t[a + 1..b],
        _ => t,
    };
    simple_name(inner.split(',').next_back().unwrap_or(inner).trim())
}

/// Writes to a context property (`_db.Orders.Add(o)`) change that entity.
pub fn mutation_edges(files: &[&FileIr], by_property: &HashMap<String, SymbolId>) -> Vec<StructEdge> {
    let mut out: BTreeMap<(String, String), StructEdge> = BTreeMap::new();
    for file in files {
        for m in file.knowledge.mutations.iter().filter(|m| m.kind == "database") {
            let Some(entity) = m.target.split('.').rev().find_map(|part| by_property.get(part)) else { continue };
            out.entry((m.owner.clone(), entity.clone())).or_insert_with(|| {
                StructEdge::new(m.owner.clone(), entity.clone(), StructEdgeKind::Mutates)
                    .with_evidence(format!("writes `{}`", m.target))
                    .with_site(m.span.clone())
            });
        }
    }
    out.into_values().collect()
}

/// Tests and what they exercise: what a test calls (exact), and test classes named after a type
/// (`OrderServiceTests` → `OrderService`, inferred).
pub fn test_edges(ctx: &Ctx, infos: &BTreeMap<SymbolId, SymbolInfo>) -> Vec<StructEdge> {
    let is_test = |id: &str| -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if infos.get(c).is_some_and(|i| i.test) {
                return true;
            }
            cur = ctx.symbols.get(c).and_then(|s| s.parent.as_deref());
        }
        false
    };
    let mut out: BTreeMap<(String, String), StructEdge> = BTreeMap::new();
    for e in ctx.edges.iter().filter(|e| e.kind == StructEdgeKind::Calls) {
        if is_test(&e.from) && !is_test(&e.to) && ctx.symbols.get(e.to.as_str()).is_some_and(|s| !s.external) {
            out.entry((e.from.clone(), e.to.clone()))
                .or_insert_with(|| StructEdge::new(e.from.clone(), e.to.clone(), StructEdgeKind::Tests).with_evidence("the test calls it"));
        }
    }
    // Modules by the last segment of their path (`src/auth` → `auth`), for test modules.
    let mut modules_by_stem: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for s in ctx.symbols.values().filter(|s| s.kind == SymbolKind::Module) {
        if let Some((_, last)) = s.qualified_name.rsplit_once('/') {
            modules_by_stem.entry(last).or_default().push(s);
        }
    }
    for id in infos.iter().filter(|(_, i)| i.test).map(|(id, _)| id) {
        let Some(sym) = ctx.symbols.get(id.as_str()) else { continue };
        if sym.kind == SymbolKind::Module {
            // `__tests__/auth.test` → the module `.../auth`.
            let last = sym.qualified_name.rsplit('/').next().unwrap_or(&sym.qualified_name);
            let Some(stem) = [".test", ".spec", ".e2e-spec", ".e2e"].iter().find_map(|s| last.strip_suffix(s)) else { continue };
            let targets: Vec<&&Symbol> = modules_by_stem
                .get(stem)
                .into_iter()
                .flatten()
                .filter(|s| s.id != *id && !is_test(&s.id))
                .collect();
            if let [target] = targets.as_slice() {
                out.entry((id.clone(), target.id.clone())).or_insert_with(|| {
                    StructEdge::new(id.clone(), target.id.clone(), StructEdgeKind::Tests)
                        .with_confidence(Confidence::Inferred)
                        .with_evidence(format!("{last} is named after it"))
                });
            }
            continue;
        }
        if !sym.kind.is_type() {
            continue;
        }
        let Some(stem) = ["Tests", "Test", "Specs", "Spec", "Should"].iter().find_map(|s| sym.name.strip_suffix(s)).filter(|s| !s.is_empty()) else { continue };
        let Some(target) = ctx.resolve(stem, id) else { continue };
        if target != *id && !is_test(&target) {
            out.entry((id.clone(), target.clone())).or_insert_with(|| {
                StructEdge::new(id.clone(), target, StructEdgeKind::Tests)
                    .with_confidence(Confidence::Inferred)
                    .with_evidence(format!("{} is named after it", sym.name))
            });
        }
    }
    out.into_values().collect()
}

/// Front-end requests linked to the back-end routes that serve them: one matching route is
/// inferred; several are unresolved (an edge to each candidate). Requests to other hosts, and
/// URLs that are only a variable, are left out.
pub fn http_edges(files: &[&FileIr], routes: &[nodex_ir::knowledge::ResolvedRoute]) -> Vec<StructEdge> {
    use super::routes::{normalize_route, url_matches};
    let mut out: BTreeMap<(String, String), StructEdge> = BTreeMap::new();
    for h in files.iter().flat_map(|f| f.knowledge.http_calls.iter()) {
        if h.url.starts_with("p:") {
            continue;
        }
        if let Some((_, rest)) = h.url.split_once("://") {
            let host = rest.split(['/', ':']).next().unwrap_or("");
            if !matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0") {
                continue;
            }
        }
        let url = normalize_route(&h.url);
        if url.trim_start_matches("{}").trim_matches('/').is_empty() {
            continue;
        }
        let candidates: Vec<&nodex_ir::knowledge::ResolvedRoute> = routes
            .iter()
            .filter(|r| r.method != "PAGE" && (r.method == h.method || r.method == "ANY") && url_matches(&url, &r.normalized))
            .collect();
        let confidence = if candidates.len() == 1 { Confidence::Inferred } else { Confidence::Unresolved };
        for r in &candidates {
            let evidence = if candidates.len() == 1 {
                format!("{} {} matches {} {} ({})", h.method, h.url, r.method, r.path, r.framework)
            } else {
                format!("{} {} matches {} routes, including {} {} ({})", h.method, h.url, candidates.len(), r.method, r.path, r.framework)
            };
            out.entry((h.owner.clone(), r.handler.clone())).or_insert_with(|| {
                StructEdge::new(h.owner.clone(), r.handler.clone(), StructEdgeKind::HttpCalls)
                    .with_confidence(confidence)
                    .with_evidence(evidence)
                    .with_site(h.span.clone())
            });
        }
    }
    out.into_values().collect()
}

/// A call made by `owner` to `target` at a site protected by `catches` (exception types handled
/// by the enclosing `catch` clauses; `*` handles every error).
pub struct CallSite {
    pub owner: SymbolId,
    pub target: SymbolId,
    pub catches: Vec<String>,
}

/// Exception types and what catches them: the base types of every type (read once from the
/// `Inherits` edges) and each type's lineage, computed on first use.
pub struct Lineage<'a> {
    bases: HashMap<&'a str, Vec<&'a str>>,
    cache: RefCell<HashMap<String, Rc<HashSet<String>>>>,
}

impl<'a> Lineage<'a> {
    pub fn new(edges: &'a [StructEdge]) -> Self {
        let mut bases: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in edges.iter().filter(|e| e.kind == StructEdgeKind::Inherits) {
            bases.entry(e.from.as_str()).or_default().push(e.to.as_str());
        }
        Self { bases, cache: RefCell::new(HashMap::new()) }
    }

    /// Simple names of `ty` and its base types, plus the universal roots (`Exception`, `Error`).
    pub fn of(&self, ctx: &Ctx, ty: &str) -> Rc<HashSet<String>> {
        if let Some(known) = self.cache.borrow().get(ty) {
            return known.clone();
        }
        let mut out: HashSet<String> = ["Exception", "Error", "Throwable", "object", "Object"].iter().map(|s| s.to_string()).collect();
        let mut cur = vec![ty];
        let mut seen = HashSet::new();
        while let Some(t) = cur.pop() {
            if !seen.insert(t) {
                continue;
            }
            out.insert(simple_name(&ctx.name(t)));
            cur.extend(self.bases.get(t).into_iter().flatten().copied());
        }
        let out = Rc::new(out);
        self.cache.borrow_mut().insert(ty.to_owned(), out.clone());
        out
    }

    /// True when one of `catches` (types of enclosing `catch` clauses; `*` for every error)
    /// handles exception type `ty` or one of its base types.
    pub fn handles(&self, ctx: &Ctx, ty: &str, catches: &[String]) -> bool {
        if catches.is_empty() {
            return false;
        }
        let lineage = self.of(ctx, ty);
        catches.iter().any(|t| t == "*" || lineage.contains(&simple_name(t)))
    }
}

/// Where each thrown exception is caught: walks up the callers of the throwing function until a
/// call site's `catch` handles the type (or its base type). A throw that reaches a route handler
/// without being caught goes to the framework's global error handlers. `caught_locally` lists the
/// (function, exception type) pairs whose every throw is inside a `catch` of that same function:
/// they never reach a caller.
pub fn caught_by_edges(
    ctx: &Ctx,
    sites: &[CallSite],
    handlers: &[GlobalHandlerFact],
    infos: &BTreeMap<SymbolId, SymbolInfo>,
    inferred_calls: &HashSet<(String, String)>,
    caught_locally: &HashSet<(String, String)>,
) -> Vec<StructEdge> {
    let mut callers: HashMap<&str, Vec<&CallSite>> = HashMap::new();
    for s in sites {
        callers.entry(s.target.as_str()).or_default().push(s);
    }
    let mut out: BTreeMap<(String, String), StructEdge> = BTreeMap::new();
    let lineages = Lineage::new(ctx.edges);
    for throw in ctx.edges.iter().filter(|e| e.kind == StructEdgeKind::Throws) {
        if caught_locally.contains(&(throw.from.clone(), throw.to.clone())) {
            continue;
        }
        let handles = |types: &[String]| lineages.handles(ctx, &throw.to, types);
        // BFS over callers: (function reached, confidence so far, depth).
        let mut queue = VecDeque::from([(throw.from.as_str(), Confidence::Exact, 0usize)]);
        let mut seen = HashSet::new();
        while let Some((callee, confidence, depth)) = queue.pop_front() {
            if !seen.insert(callee) || depth > 6 {
                continue;
            }
            let at_route = infos.get(callee).and_then(|i| i.entry.as_ref()).is_some_and(|e| e.kind == EntryKind::Route);
            if at_route {
                for h in handlers {
                    if h.handler == callee {
                        continue;
                    }
                    out.entry((throw.from.clone(), h.handler.clone())).or_insert_with(|| {
                        StructEdge::new(throw.from.clone(), h.handler.clone(), StructEdgeKind::CaughtBy)
                            .with_confidence(Confidence::Inferred)
                            .with_evidence(format!("{} reaches route handler {} uncaught; the {} global handler takes it", ctx.name(&throw.to), ctx.name(callee), h.framework))
                            .with_site(h.span.clone())
                    });
                }
                continue;
            }
            for site in callers.get(callee).into_iter().flatten() {
                let hop = if inferred_calls.contains(&(site.owner.clone(), site.target.clone())) { Confidence::Inferred } else { Confidence::Exact };
                let confidence = confidence.weakest(hop);
                if handles(&site.catches) {
                    out.entry((throw.from.clone(), site.owner.clone())).or_insert_with(|| {
                        StructEdge::new(throw.from.clone(), site.owner.clone(), StructEdgeKind::CaughtBy)
                            .with_confidence(confidence)
                            .with_evidence(format!("{} is caught in {} (catch {})", ctx.name(&throw.to), ctx.name(&site.owner), site.catches.join(" | ")))
                    });
                } else {
                    queue.push_back((site.owner.as_str(), confidence, depth + 1));
                }
            }
        }
    }
    out.into_values().collect()
}
