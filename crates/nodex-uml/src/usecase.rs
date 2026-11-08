//! Use case diagrams: what the system lets people (and other systems) do, as a Mermaid flowchart.
//!
//! Use cases are the entry points of the knowledge graph: routes, UI actions, commands, jobs,
//! message handlers, event handlers, exported functions and `main` (a web host's `main` is left
//! out when the same component serves routes). Each gets a plain name (its doc comment, or its
//! handler and route read as words: `POST /api/orders/{id}/cancel` handled by `Cancel` → "Cancel
//! order") and the actor who starts it:
//!
//! - routes: one actor per role the code requires (`[Authorize(Roles = "Admin")]`, Nest
//!   `@Roles`, `requireRole("admin")`), "Authenticated user" when it only requires a login,
//!   "Visitor" when it allows anonymous calls, "API client" otherwise; role actors specialize
//!   "Authenticated user";
//! - UI actions: "User"; commands and `main`: "Operator"; jobs: "Scheduler"; messages: "Message
//!   broker"; event handlers: "External system"; exports: "Platform".
//!
//! Use cases are grouped by component (build unit). Relations between them are read from the
//! code: a UI action that ends in an HTTP request to a route `«include»`s the route's use case;
//! an operation that several use cases call (a service method with an action name) becomes an
//! `«include»`d use case; one that a use case calls only under a condition `«extend»`s it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nodex_ir::knowledge::{Confidence, EntryKind};
use nodex_ir::{Concept, StructEdgeKind, Symbol, SymbolKind};

use crate::calls::{Callee, Resolver};
use crate::flowtree::{Block, call_sites, tree};
use crate::index::Index;
use crate::mermaid::{Ids, class_defs, label};
use crate::scope::Scoped;
use crate::{UmlDiagram, UmlElement, UmlInput, UmlKind, UmlOptions, UmlRelation, empty};

const MAX_CASES: usize = 120;
/// Words that already say what a use case does.
const VERBS: &[&str] = &[
    "cancel", "create", "delete", "remove", "purge", "restart", "pay", "ship", "approve", "reject", "submit", "send", "update", "upload", "download", "login",
    "logout", "register", "reset", "search", "export", "import", "sync", "refresh", "start", "stop", "run", "process", "validate", "check", "confirm",
    "archive", "restore", "add", "list", "view", "get", "set", "save", "edit", "publish", "invite", "join", "leave", "sign", "verify", "close", "open",
    "lock", "unlock", "assign", "book", "order", "buy", "sell", "transfer", "charge", "refund", "notify", "subscribe", "unsubscribe", "call", "track",
];
const CLOSURE_DEPTH: usize = 3;

struct Case {
    symbol: String,
    name: String,
    /// The route or trigger, shown under the name.
    sub: String,
    kind: EntryKind,
    actors: Vec<(String, String)>,
    unit: String,
    group: Option<String>,
    effects: Vec<String>,
}

/// `CancelOrderAsync` → `["cancel", "order"]`; `find_one` → `["find", "one"]`.
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == '-' || c == ' ' || c == '.' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let boundary = c.is_uppercase() && i > 0 && (chars[i - 1].is_lowercase() || chars.get(i + 1).is_some_and(|n| n.is_lowercase()) && chars[i - 1].is_uppercase());
        if boundary && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    let mut out: Vec<String> = out.into_iter().map(|w| w.to_lowercase()).collect();
    out.retain(|w| !matches!(w.as_str(), "async" | "handler" | "command" | "endpoint" | "action"));
    if out.first().is_some_and(|w| matches!(w.as_str(), "handle" | "on" | "do")) && out.len() > 1 {
        out.remove(0);
    }
    out
}

fn sentence(words: &[String]) -> String {
    let s = words.join(" ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

fn singular(w: &str) -> String {
    if let Some(s) = w.strip_suffix("ies") {
        format!("{s}y")
    } else if w.ends_with("ses") || w.ends_with("xes") {
        w[..w.len() - 2].to_owned()
    } else if let Some(s) = w.strip_suffix('s').filter(|s| !s.ends_with('s')) {
        s.to_owned()
    } else {
        w.to_owned()
    }
}

/// The resource a route path is about: its last literal segment, singular when an id follows.
fn resource(path: &str) -> Option<String> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let is_param = |s: &str| s.starts_with('{') || s.starts_with(':') || s.starts_with('<') || s.starts_with('[');
    let mut found: Option<(usize, &str)> = None;
    for (i, s) in segs.iter().enumerate() {
        if !is_param(s) && !matches!(s.to_ascii_lowercase().as_str(), "api" | "v1" | "v2" | "v3") {
            found = Some((i, s));
        }
    }
    let (i, s) = found?;
    let s = s.to_ascii_lowercase();
    // `/orders/{id}/cancel`: the action is the last segment, the resource the one before.
    let (i, s) = if i > 0 && segs.get(i - 1).is_some_and(|p| is_param(p)) && i >= 2 { (i - 2, segs[i - 2].to_ascii_lowercase()) } else { (i, s) };
    let followed_by_id = segs.get(i + 1).is_some_and(|n| is_param(n));
    Some(if followed_by_id { singular(&s) } else { s }).map(|r| r.replace(['-', '_'], " "))
}

/// A plain name for a use case.
fn case_name(s: &Symbol, kind: EntryKind, entry_label: &str, route: Option<(&str, &str)>) -> String {
    if let Some(doc) = s.doc.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        let first = doc.split(['.', '\n']).next().unwrap_or(doc).trim();
        if first.len() <= 60 && !first.is_empty() {
            return first.to_owned();
        }
    }
    if kind == EntryKind::Ui {
        // `click "Refresh"` → `Refresh`; `submit <form>` → `Submit form`.
        let text = entry_label.split_once('"').map(|(_, r)| r.trim_end_matches('"').to_owned());
        return match text {
            Some(t) if !t.trim().is_empty() => sentence(&t.split_whitespace().map(str::to_owned).collect::<Vec<_>>()),
            _ => sentence(&entry_label.replace(['<', '>'], "").split_whitespace().map(str::to_owned).collect::<Vec<_>>()),
        };
    }
    let name = s.name.split('(').next().unwrap_or(&s.name);
    let lambda = name.contains("callback") || name.contains("=>") || name.contains("(module)") || name.contains(' ');
    let mut w = if lambda { Vec::new() } else { words(name) };
    if let Some((method, path)) = route {
        let res = resource(path);
        let generic = w.is_empty() || matches!(w[0].as_str(), "index" | "get" | "post" | "put" | "patch" | "delete" | "find" | "fetch" | "list" | "read" | "show" | "handle");
        let verb = match method {
            "GET" if path.split('/').next_back().is_some_and(|l| l.starts_with('{') || l.starts_with(':')) => "view",
            "GET" => "list",
            "POST" => "create",
            "PUT" | "PATCH" => "update",
            "DELETE" => "delete",
            _ => "call",
        };
        if w.is_empty() {
            w = vec![verb.to_owned()];
        } else if generic && w.len() <= 2 && matches!(w.get(1).map(String::as_str), None | Some("one" | "all" | "by" | "many" | "id")) {
            w = vec![if w[0] == "find" && w.get(1).is_some_and(|x| x == "all") { "list".to_owned() } else if matches!(w[0].as_str(), "find" | "get" | "read" | "show" | "fetch") { verb.to_owned() } else { w[0].clone() }];
        }
        // A noun alone (`Reports`, `Health`): say what is done with it.
        if w.len() == 1 && !VERBS.contains(&w[0].as_str()) {
            w.insert(0, verb.to_owned());
            if verb == "list" {
                w[0] = "view".to_owned();
            }
        }
        // A verb alone (`Cancel`, `Purge`): say what it is about.
        if w.len() == 1
            && let Some(r) = res
            && !w[0].contains(&r)
            && !r.contains(&w[0])
        {
            // One thing is created, updated or deleted at a time.
            let r = if method == "GET" { r } else { singular(&r) };
            w.push(r);
        }
    }
    if w.is_empty() {
        return entry_label.to_owned();
    }
    sentence(&w)
}

fn actors_of(index: &Index, s: &Symbol, kind: EntryKind) -> Vec<(String, String)> {
    let info = index.info.get(s.id.as_str());
    let auth = info.and_then(|i| i.auth.as_ref());
    let needs_auth = info.is_some_and(|i| i.capabilities.iter().any(|c| c == "AUTH_REQUIRED"));
    match kind {
        EntryKind::Route => match auth {
            Some(a) if a.anonymous => vec![("Visitor".into(), "allows anonymous callers".into())],
            Some(a) if !a.roles.is_empty() => a.roles.iter().map(|r| (title(r), format!("requires role {r}"))).collect(),
            Some(a) if !a.policies.is_empty() => vec![("Authenticated user".into(), format!("requires policy {}", a.policies.join(", ")))],
            _ if needs_auth => vec![("Authenticated user".into(), "requires an authenticated caller".into())],
            _ => vec![("API client".into(), "public route".into())],
        },
        EntryKind::Ui => vec![("User".into(), "a UI action".into())],
        EntryKind::Main | EntryKind::Command => vec![("Operator".into(), "runs the program or a command".into())],
        EntryKind::Job => vec![("Scheduler".into(), "a scheduled or background job".into())],
        EntryKind::Message => vec![("Message broker".into(), "handles a message".into())],
        EntryKind::EventHandler => vec![("External system".into(), "reacts to an event".into())],
        EntryKind::Export => vec![("Platform".into(), "exported to a host".into())],
        _ => vec![("System".into(), String::new())],
    }
}

fn title(role: &str) -> String {
    let mut c = role.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => role.to_owned(),
    }
}

fn unit_of(input: &UmlInput, file: &str) -> String {
    let unit = input.units.iter().filter(|u| u.root.is_empty() || file.starts_with(&format!("{}/", u.root))).max_by_key(|u| u.root.len());
    match unit {
        Some(u) if !u.root.is_empty() || input.units.len() == 1 => u.name.clone(),
        _ => file.split_once('/').map(|(t, _)| t.to_owned()).unwrap_or_else(|| input.project.to_owned()),
    }
}

/// Project functions a use case reaches, with whether each call was conditional at the first
/// level (for `«extend»`).
fn closure(index: &Index, input: &UmlInput, resolver: &Resolver, root: &str) -> (BTreeMap<String, bool>, BTreeSet<String>) {
    let mut reached: BTreeMap<String, bool> = BTreeMap::new();
    let mut http = BTreeSet::new();
    let mut frontier = vec![root.to_owned()];
    let mut seen = BTreeSet::from([root.to_owned()]);
    for depth in 0..CLOSURE_DEPTH {
        let mut next = Vec::new();
        for f in frontier {
            for e in index.graph.edges.iter().filter(|e| e.kind == StructEdgeKind::HttpCalls && e.from == f) {
                http.insert(e.to.clone());
            }
            let (Some(sym), Some(flow)) = (index.get(&f), input.flows.flow(&f)) else { continue };
            let body = tree(&flow);
            let mut found = Vec::new();
            walk(&body, false, &mut found);
            for (node_id, conditional) in found {
                let Some(node) = flow.node(&node_id) else { continue };
                for site in call_sites(node) {
                    if let Some(Callee::Project { target, .. }) = resolver.resolve(sym, &flow, node, &site)
                        && index.get(&target).is_some_and(|t| t.has_flow && !t.kind.is_type())
                    {
                        let cond = depth == 0 && conditional;
                        reached.entry(target.clone()).and_modify(|c| *c &= cond).or_insert(cond);
                        if seen.insert(target.clone()) {
                            next.push(target);
                        }
                    }
                }
            }
        }
        frontier = next;
    }
    (reached, http)
}

/// Statements of a block, with whether a condition guards them.
fn walk(block: &Block, conditional: bool, out: &mut Vec<(String, bool)>) {
    for s in &block.steps {
        out.push((s.node.id.clone(), conditional));
        let branchy = matches!(s.node.concept, Concept::Branch | Concept::Match);
        for a in &s.arms {
            walk(&a.block, conditional || branchy, out);
        }
    }
}

/// Entry points of a scope that are use cases (for availability).
pub(crate) fn count(index: &Index, scoped: &Scoped, input: &UmlInput) -> usize {
    input
        .knowledge
        .symbols
        .iter()
        .filter(|i| i.entry.as_ref().is_some_and(|e| !matches!(e.kind, EntryKind::Framework | EntryKind::Test | EntryKind::ServerStart)))
        .filter(|i| scoped.symbols.contains(&i.symbol) && !index.is_test(&i.symbol))
        .count()
}

pub fn build(index: &Index, scoped: &Scoped, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut d = empty(UmlKind::UseCase);
    // Units that serve routes: their `main` is a web host, not a use case.
    let mut route_units = BTreeSet::new();
    for r in &input.knowledge.routes {
        if let Some(f) = index.file_of(&r.handler) {
            route_units.insert(unit_of(input, f));
        }
    }
    let mut cases: Vec<Case> = Vec::new();
    for info in &input.knowledge.symbols {
        let Some(entry) = &info.entry else { continue };
        if matches!(entry.kind, EntryKind::Framework | EntryKind::Test | EntryKind::ServerStart) || !scoped.symbols.contains(&info.symbol) {
            continue;
        }
        let Some(s) = index.get(&info.symbol) else { continue };
        if opts.hide_tests && index.is_test(&s.id) {
            continue;
        }
        let file = s.origins.first().map(|o| o.file.as_str()).unwrap_or("");
        let unit = unit_of(input, file);
        if entry.kind == EntryKind::Main && route_units.contains(&unit) {
            continue;
        }
        let route = input.knowledge.routes.iter().find(|r| r.handler == s.id);
        let name = case_name(s, entry.kind, &entry.label, route.map(|r| (r.method.as_str(), r.path.as_str())));
        let sub = match route {
            Some(r) => format!("{} {}", r.method, r.path),
            // `click "Refresh" in OrderList`.
            None if entry.kind == EntryKind::Ui => {
                let mut outer = s;
                while let Some(p) = outer.parent.as_deref().and_then(|p| index.get(p)).filter(|p| matches!(p.kind, SymbolKind::Function | SymbolKind::Method) || p.kind.is_type()) {
                    outer = p;
                }
                if outer.id == s.id { entry.label.clone() } else { format!("{} in {}", entry.label, outer.name) }
            }
            None => entry.label.clone(),
        };
        let group = index.owner_type(&s.id).filter(|t| t.id != s.id && t.kind.is_type()).map(|t| t.name.clone());
        cases.push(Case { symbol: s.id.clone(), name, sub, kind: entry.kind, actors: actors_of(index, s, entry.kind), unit, group, effects: Vec::new() });
    }
    if cases.is_empty() {
        d.warnings.push("No entry points in this scope: no routes, UI actions, commands, jobs, message or event handlers, exports or main.".into());
        return d;
    }
    cases.sort_by(|a, b| (&a.unit, &a.group, &a.name).cmp(&(&b.unit, &b.group, &b.name)));
    if cases.len() > MAX_CASES {
        d.truncated = true;
        d.stats.hidden = (cases.len() - MAX_CASES) as u32;
        d.warnings.push(format!("{} use cases; the first {MAX_CASES} are drawn. Choose a folder to see the others.", cases.len()));
        cases.truncate(MAX_CASES);
    }

    // What each use case reaches: operations shared by several, HTTP requests, data changes.
    let resolver = Resolver::new(index);
    let entry_ids: BTreeSet<&str> = input.knowledge.symbols.iter().filter(|i| i.entry.is_some()).map(|i| i.symbol.as_str()).collect();
    let mut reach: Vec<BTreeMap<String, bool>> = Vec::new();
    let mut https: Vec<BTreeSet<String>> = Vec::new();
    for c in cases.iter_mut() {
        let (r, h) = closure(index, input, &resolver, &c.symbol);
        let mut mutated = BTreeSet::new();
        for f in r.keys().chain(std::iter::once(&c.symbol)) {
            for e in index.graph.edges.iter().filter(|e| e.kind == StructEdgeKind::Mutates && e.from == *f) {
                if let Some(t) = index.get(&e.to) {
                    mutated.insert(t.name.clone());
                }
            }
        }
        c.effects = mutated.into_iter().map(|m| format!("✎ {m}")).collect();
        reach.push(r);
        https.push(h);
    }
    let mut shared: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in reach.iter().enumerate() {
        for f in r.keys() {
            shared.entry(f.clone()).or_default().push(i);
        }
    }
    let action_like = |s: &Symbol| {
        let w = words(&s.name);
        let first = w.first().map(String::as_str).unwrap_or("");
        s.kind == SymbolKind::Method
            && !entry_ids.contains(s.id.as_str())
            && w.len() >= 2
            && !matches!(first, "get" | "set" | "is" | "has" | "to" | "log" | "map" | "format" | "build" | "create" | "new" | "find" | "read" | "count" | "describe")
            && !index.info.get(s.id.as_str()).is_some_and(|i| i.capabilities.iter().all(|c| c == "LOGGING") && !i.capabilities.is_empty())
    };
    let mut included: Vec<(String, Vec<usize>)> = shared
        .into_iter()
        .filter(|(f, users)| users.len() >= 2 && index.get(f).is_some_and(action_like))
        .collect();
    included.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    included.truncate(8);

    // Drawing.
    let mut ids = Ids::new("n");
    let mut out = String::from("flowchart LR\n");
    let mut elements = Vec::new();
    let mut relations = Vec::new();
    let mut classes: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut actors: BTreeMap<String, String> = BTreeMap::new();
    for c in &cases {
        for (a, _) in &c.actors {
            actors.entry(a.clone()).or_insert_with(|| ids.id(&format!("actor:{a}")));
        }
    }
    for (a, id) in &actors {
        out.push_str(&format!("  {id}[\"👤 {}\"]\n", label(a, 40)));
        classes.entry("actor").or_default().push(id.clone());
        elements.push(UmlElement { mermaid_id: id.clone(), symbol: None, label: a.clone(), kind: "actor".into(), span: None, context: false, detail: None });
    }
    let mut by_unit: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, c) in cases.iter().enumerate() {
        by_unit.entry(c.unit.as_str()).or_default().push(i);
    }
    let incl_unit = |f: &str| index.file_of(f).map(|file| unit_of(input, file)).unwrap_or_default();
    let mut g = 0;
    for (unit, list) in &by_unit {
        out.push_str(&format!("  subgraph g{g} [\"{}\"]\n", label(unit, 50)));
        g += 1;
        let grouped = list.len() > 6;
        let mut groups: BTreeMap<Option<&str>, Vec<usize>> = BTreeMap::new();
        for &i in list {
            groups.entry(if grouped { cases[i].group.as_deref() } else { None }).or_default().push(i);
        }
        for (group, items) in &groups {
            let pad = if group.is_some() { "      " } else { "    " };
            if let Some(name) = group {
                out.push_str(&format!("    subgraph g{g} [\"{}\"]\n", label(name, 50)));
                g += 1;
                out.push_str("      direction TB\n");
            }
            for &i in items {
                let c = &cases[i];
                let id = ids.id(&c.symbol);
                let mut parts = vec![label(&c.name, 50), label(&c.sub, 50)];
                if !c.effects.is_empty() {
                    parts.push(label(&c.effects.join(" "), 50));
                }
                out.push_str(&format!("{pad}{id}([\"{}\"])\n", parts.join("<br/>")));
                let sym = index.get(&c.symbol);
                elements.push(UmlElement {
                    mermaid_id: id,
                    symbol: Some(c.symbol.clone()),
                    label: c.name.clone(),
                    kind: "use case".into(),
                    span: sym.and_then(|s| s.origins.first().cloned()),
                    context: false,
                    detail: Some(format!("{} · {:?}", c.sub, c.kind)),
                });
            }
            if group.is_some() {
                out.push_str("    end\n");
            }
        }
        // Included operations of this component.
        for (f, _) in included.iter().filter(|(f, _)| incl_unit(f) == *unit) {
            let Some(s) = index.get(f) else { continue };
            let id = ids.id(f);
            out.push_str(&format!("    {id}([\"{}\"])\n", label(&sentence(&words(&s.name)), 50)));
            classes.entry("incl").or_default().push(id.clone());
            elements.push(UmlElement {
                mermaid_id: id,
                symbol: Some(f.clone()),
                label: sentence(&words(&s.name)),
                kind: "included use case".into(),
                span: s.origins.first().cloned(),
                context: false,
                detail: Some("an operation several use cases share".into()),
            });
        }
        out.push_str("  end\n");
    }
    // Included operations outside every drawn component.
    for (f, _) in included.iter().filter(|(f, _)| !by_unit.contains_key(incl_unit(f).as_str())) {
        let Some(s) = index.get(f) else { continue };
        let id = ids.id(f);
        out.push_str(&format!("  {id}([\"{}\"])\n", label(&sentence(&words(&s.name)), 50)));
        classes.entry("incl").or_default().push(id.clone());
        elements.push(UmlElement { mermaid_id: id, symbol: Some(f.clone()), label: sentence(&words(&s.name)), kind: "included use case".into(), span: s.origins.first().cloned(), context: true, detail: None });
    }

    // Actors and their use cases.
    for c in &cases {
        let uc = ids.id(&c.symbol);
        for (a, why) in &c.actors {
            let aid = &actors[a];
            out.push_str(&format!("  {aid} --- {uc}\n"));
            relations.push(UmlRelation { from: aid.clone(), to: uc.clone(), kind: "association".into(), label: None, confidence: None, evidence: Some(why.clone()), site: None, count: 1 });
        }
    }
    // Roles are kinds of authenticated users.
    if let Some(user) = actors.get("Authenticated user") {
        for (a, id) in &actors {
            let role = cases.iter().any(|c| c.kind == EntryKind::Route && c.actors.iter().any(|(x, w)| x == a && w.starts_with("requires role")));
            if role {
                out.push_str(&format!("  {id} -.->|\"is a\"| {user}\n"));
                relations.push(UmlRelation { from: id.clone(), to: user.clone(), kind: "generalization".into(), label: Some("is a".into()), confidence: None, evidence: Some(format!("{a} is a role of an authenticated user")), site: None, count: 1 });
            }
        }
    }
    // UI → API: the UI action's code sends a request a route of the project serves.
    let by_symbol: HashMap<&str, usize> = cases.iter().enumerate().map(|(i, c)| (c.symbol.as_str(), i)).collect();
    let mut drawn = BTreeSet::new();
    for (i, h) in https.iter().enumerate() {
        for target in h {
            if let Some(&j) = by_symbol.get(target.as_str())
                && i != j
                && drawn.insert((i, j))
            {
                let (a, b) = (ids.id(&cases[i].symbol), ids.id(&cases[j].symbol));
                out.push_str(&format!("  {a} -.->|\"«include»\"| {b}\n"));
                relations.push(UmlRelation {
                    from: a,
                    to: b,
                    kind: "include".into(),
                    label: Some("«include» over HTTP".into()),
                    confidence: Some(Confidence::Inferred),
                    evidence: Some(format!("{} sends a request that {} serves", cases[i].name, cases[j].sub)),
                    site: None,
                    count: 1,
                });
            }
        }
    }
    // Shared operations: «include», or «extend» when only reached under a condition.
    for (f, users) in &included {
        let target = ids.id(f);
        for &i in users {
            let uc = ids.id(&cases[i].symbol);
            let conditional = reach[i].get(f).copied().unwrap_or(false);
            let (text, from, to) = if conditional { ("«extend»", target.clone(), uc.clone()) } else { ("«include»", uc.clone(), target.clone()) };
            out.push_str(&format!("  {from} -.->|\"{text}\"| {to}\n"));
            relations.push(UmlRelation {
                from,
                to,
                kind: if conditional { "extend".into() } else { "include".into() },
                label: Some(text.into()),
                confidence: Some(Confidence::Inferred),
                evidence: Some(format!(
                    "{} calls {}{}",
                    cases[i].name,
                    index.get(f).map(|s| s.name.as_str()).unwrap_or(""),
                    if conditional { " only under a condition" } else { "" }
                )),
                site: None,
                count: 1,
            });
        }
    }
    let used: Vec<&str> = classes.keys().copied().collect();
    class_defs(&mut out, &used);
    for (c, list) in &classes {
        out.push_str(&format!("  class {} {c}\n", list.join(",")));
    }
    d.mermaid = out;
    d.elements = elements;
    d.relations = relations;
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_as_words() {
        assert_eq!(words("CancelOrderAsync"), vec!["cancel", "order"]);
        assert_eq!(words("handleSubmit"), vec!["submit"]);
        assert_eq!(words("find_one"), vec!["find", "one"]);
        assert_eq!(words("HTTPServer"), vec!["http", "server"]);
        assert_eq!(resource("/api/Orders/{id}/cancel").as_deref(), Some("order"));
        assert_eq!(resource("/api/invoices/:id").as_deref(), Some("invoice"));
        assert_eq!(resource("/api/orders").as_deref(), Some("orders"));
        assert_eq!(singular("categories"), "category");
    }
}
