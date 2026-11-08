//! State diagrams: how a behavior changes, as a Mermaid `stateDiagram-v2`.
//!
//! Machines come from two kinds of evidence. **Declared** machines (XState, C# Stateless, Unity
//! Animator) are facts of the knowledge graph and exact. **Detected** machines are read from the
//! code, best evidence first:
//!
//! | detector | the state is | transitions |
//! |---|---|---|
//! | enum field | a field or property of an enum type | assignments in the type's methods |
//! | entity status | the `Status`/`State` field of a data model | assignments anywhere (`order.Status = …`), triggered by the route that reaches them |
//! | React state | `const [s, setS] = useState("idle")` | `setS("loading")` calls |
//! | State pattern | a field whose type is an interface with several implementations | `SetState(new B())` inside implementation `A` |
//! | literal field | a field holding string or number literals (TypeScript unions) | assignments |
//! | flags | up to three boolean fields (`_connected`, `_disposed`) | their combinations, by method |
//! | loop variable | a local reassigned inside a loop that switches on it (lexers, game loops) | assignments in the body |
//! | type-state | a generic Rust type whose methods consume `self` (`Door<Closed>` → `Door<Open>`) | those methods |
//!
//! Transitions are read path by path (see [`engine`]): guard clauses, `switch` cases and helper
//! guards decide the source states; other conditions become guards.

mod engine;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nodex_ir::knowledge::{Confidence, EntryKind};
use nodex_ir::{Concept, Expr, Language, Span, StructEdgeKind, Symbol, SymbolKind};

use crate::calls::Resolver;
use crate::flowtree::{Block, call_sites, tree};
use crate::index::Index;
use crate::mermaid::{class_defs, shorten};
use crate::scope::Scoped;
use crate::types::analyze;
use crate::{UmlCandidate, UmlDiagram, UmlElement, UmlInput, UmlKind, UmlOptions, UmlRelation, empty};
use engine::{Matcher, Path, Var, Walker, literal_value};

/// Where a machine comes from (cheap to list; the machine is read on demand).
#[derive(Clone, Debug, PartialEq)]
enum Spec {
    Declared(usize),
    Field { owner: String, field: String },
    Entity { model: String, field: String },
    Hook { function: String, state: String, setter: String },
    Pattern { owner: String, field: String, iface: String },
    Flags { owner: String, fields: Vec<String> },
    Loop { function: String, local: String },
    TypeState { owner: String },
}

struct Candidate {
    id: String,
    label: String,
    detail: String,
    score: f32,
    spec: Spec,
}

#[derive(Clone, Debug)]
struct Trans {
    /// `None`: from any state.
    from: Option<String>,
    to: String,
    trigger: Option<String>,
    guard: Option<String>,
    confidence: Confidence,
    evidence: String,
    site: Option<Span>,
}

#[derive(Default)]
struct Machine {
    title: String,
    detector: &'static str,
    /// States in order; nested ones as `Parent.Child`.
    states: Vec<String>,
    /// Symbols of states (enum members, implementation classes).
    symbols: HashMap<String, String>,
    initial: Vec<String>,
    finals: BTreeSet<String>,
    trans: Vec<Trans>,
    confidence: Option<Confidence>,
    notes: Vec<String>,
    /// Span of the declaration, for states without a symbol.
    span: Option<Span>,
    owner: Option<String>,
}

const TERMINAL: &[&str] = &[
    "cancelled", "canceled", "closed", "completed", "complete", "done", "finished", "ended", "archived", "deleted", "disposed", "terminated", "rejected", "expired",
    "refunded", "delivered", "dead", "destroyed", "removed",
];
const TERMINAL_METHODS: &[&str] = &["dispose", "close", "complete", "finish", "terminate", "destroy", "die", "kill", "delete", "shutdown", "stop", "end"];
const FLAG_WORDS: &[&str] = &[
    "connected", "disposed", "running", "open", "opened", "closed", "active", "enabled", "started", "stopped", "loaded", "ready", "busy", "visible", "paused",
    "initialized", "completed", "done", "locked", "dirty", "alive", "dead", "playing", "grounded",
];

fn bare(name: &str) -> String {
    name.split('(').next().unwrap_or(name).trim_end_matches(".get").trim_end_matches(".set").to_owned()
}

/// The value an initializer text names (`= OrderStatus.Draft;` → `Draft`, `new Red()` → `Red`).
fn value_of_text(text: &str) -> Option<String> {
    let t = text.trim().trim_end_matches(';').trim();
    let t = t.trim_start_matches("new ").trim_end_matches("()").trim();
    let t = t.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    let last = t.rsplit(['.', ':']).next().unwrap_or(t).trim();
    (!last.is_empty() && last.chars().all(|c| c.is_alphanumeric() || c == '_')).then(|| last.to_owned())
}

pub fn build(index: &Index, scoped: &Scoped, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut d = empty(UmlKind::State);
    let resolver = Resolver::new(index);
    let list = candidates(index, scoped, input, &resolver);
    d.candidates = list.iter().map(|c| UmlCandidate { id: c.id.clone(), label: c.label.clone(), detail: Some(c.detail.clone()), score: c.score }).collect();
    // The chosen machine, or the best one that turns out to have transitions.
    let mut order: Vec<&Candidate> = list.iter().collect();
    if let Some(f) = &opts.focus
        && let Some(pos) = order.iter().position(|c| c.id == *f)
    {
        let c = order.remove(pos);
        order.insert(0, c);
    }
    let mut machine = None;
    for (i, c) in order.iter().enumerate().take(8) {
        let m = read(index, input, &resolver, &c.spec);
        let useful = m.states.len() >= 2 && !m.trans.is_empty();
        let chosen = i == 0 && opts.focus.as_deref() == Some(c.id.as_str());
        if useful || chosen {
            machine = Some(m);
            break;
        }
    }
    let Some(m) = machine else {
        d.warnings.push(
            "No state machine found here: no declared machine (XState, Stateless, Animator), and no enum, literal, flag or State-pattern field that the code changes. Try a type or a folder with one."
                .into(),
        );
        return d;
    };
    render(&mut d, index, &m, opts);
    d
}

/// The machines a scope offers (label, score), best first, for availability.
pub(crate) fn offered(index: &Index, scoped: &Scoped, input: &UmlInput) -> Vec<(String, f32)> {
    let resolver = Resolver::new(index);
    candidates(index, scoped, input, &resolver).into_iter().map(|c| (c.label, c.score)).collect()
}

/// Lists the machines of a scope, best evidence first.
fn candidates(index: &Index, scoped: &Scoped, input: &UmlInput, resolver: &Resolver) -> Vec<Candidate> {
    let mut out = Vec::new();
    let in_scope = |id: &str| scoped.symbols.contains(id) || index.owner_type(id).is_some_and(|t| scoped.types.contains(&t.id));
    for (i, m) in input.knowledge.state_machines.iter().enumerate() {
        if in_scope(&m.owner) {
            out.push(Candidate {
                id: format!("state:declared:{}:{}", m.owner, m.name),
                label: format!("{} ({})", m.name, m.framework),
                detail: format!("declared with {}: {} states", m.framework, m.states.len()),
                score: 110.0,
                spec: Spec::Declared(i),
            });
        }
    }
    for t in scoped.types.iter().filter_map(|t| index.get(t)) {
        if index.is_test(&t.id) || !matches!(t.kind, SymbolKind::Class | SymbolKind::Struct | SymbolKind::Record) {
            continue;
        }
        let fields: Vec<&Symbol> = index.children.get(t.id.as_str()).into_iter().flatten().copied().filter(|m| matches!(m.kind, SymbolKind::Field | SymbolKind::Property)).collect();
        let methods = index.children.get(t.id.as_str()).into_iter().flatten().filter(|m| m.has_flow).count();
        let is_model = index.models.contains_key(t.id.as_str());
        let mut flags = Vec::new();
        for f in &fields {
            let Some(ty) = f.type_text.as_deref() else { continue };
            if f.flavors.iter().any(|x| x == "const" || x == "static") {
                continue;
            }
            let shape = analyze(ty);
            let target = shape.as_ref().filter(|s| !s.many).and_then(|s| resolver.type_named(&s.target, t));
            let name = f.name.trim_start_matches('_');
            let statish = name.to_ascii_lowercase().contains("state") || name.to_ascii_lowercase().contains("status") || name.to_ascii_lowercase().contains("mode") || name.to_ascii_lowercase().contains("phase");
            match target {
                Some(e) if e.kind == SymbolKind::Enum => {
                    let members = index.children.get(e.id.as_str()).map(|c| c.len()).unwrap_or(0);
                    if members >= 2 {
                        let entity = is_model && statish;
                        out.push(Candidate {
                            id: format!("state:field:{}:{}", t.id, f.name),
                            label: format!("{}.{}", t.name, f.name),
                            detail: format!("{} field of {} ({members} values){}", e.name, t.name, if entity { ", stored" } else { "" }),
                            score: 90.0 + if statish { 5.0 } else { 0.0 } + methods.min(10) as f32,
                            spec: Spec::Field { owner: t.id.clone(), field: f.name.clone() },
                        });
                        if entity {
                            out.push(Candidate {
                                id: format!("state:entity:{}:{}", t.id, f.name),
                                label: format!("{} {} (stored)", t.name, f.name),
                                detail: format!("life cycle of stored {} records: every function that changes {}", t.name, f.name),
                                score: 105.0,
                                spec: Spec::Entity { model: t.id.clone(), field: f.name.clone() },
                            });
                        }
                    }
                }
                Some(i) if (i.kind.is_contract() || i.flavors.iter().any(|x| x == "abstract")) && implementations(index, &i.id).len() >= 2 => {
                    out.push(Candidate {
                        id: format!("state:pattern:{}:{}", t.id, f.name),
                        label: format!("{}.{} (State pattern)", t.name, f.name),
                        detail: format!("{} implementations of {}", implementations(index, &i.id).len(), i.name),
                        score: 80.0,
                        spec: Spec::Pattern { owner: t.id.clone(), field: f.name.clone(), iface: i.id.clone() },
                    });
                }
                _ if ty == "bool" || ty == "boolean" => {
                    let low = name.to_ascii_lowercase();
                    let low = low.trim_start_matches("is").trim_start_matches("has");
                    if FLAG_WORDS.iter().any(|w| low.ends_with(w)) {
                        flags.push(f.name.clone());
                    }
                }
                _ if (ty.contains('|') && ty.contains('"')) || (statish && matches!(ty, "string" | "String" | "&str" | "int" | "number")) => {
                    out.push(Candidate {
                        id: format!("state:field:{}:{}", t.id, f.name),
                        label: format!("{}.{}", t.name, f.name),
                        detail: format!("{} values of {}", if ty.contains('"') { "literal" } else { ty }, t.name),
                        score: 65.0 + if statish { 5.0 } else { 0.0 },
                        spec: Spec::Field { owner: t.id.clone(), field: f.name.clone() },
                    });
                }
                _ => {}
            }
        }
        if !flags.is_empty() {
            flags.truncate(3);
            out.push(Candidate {
                id: format!("state:flags:{}", t.id),
                label: format!("{} flags", t.name),
                detail: format!("combinations of {}", flags.join(", ")),
                score: 55.0,
                spec: Spec::Flags { owner: t.id.clone(), fields: flags },
            });
        }
        // Rust type-state: `Door<S>` with methods that consume `self` and return `Door<Other>`.
        if t.language == Language::Rust && t.signature.as_deref().is_some_and(|s| s.contains('<')) {
            let moves = index.children.get(t.id.as_str()).into_iter().flatten().filter(|m| m.flavors.iter().any(|f| f == "self") && m.type_text.as_deref().is_some_and(|r| r.starts_with(&format!("{}<", t.name)))).count();
            if moves > 0 {
                out.push(Candidate {
                    id: format!("state:typestate:{}", t.id),
                    label: format!("{} (type-state)", t.name),
                    detail: format!("{moves} methods move a {} to another state type", t.name),
                    score: 50.0,
                    spec: Spec::TypeState { owner: t.id.clone() },
                });
            }
        }
    }
    // React state and loop variables need the function bodies: only for a reasonable scope.
    let functions: Vec<&Symbol> = scoped.symbols.iter().filter_map(|s| index.get(s)).filter(|s| s.has_flow && !index.is_test(&s.id)).collect();
    if functions.len() <= 200 {
        for f in functions {
            let react = f.flavors.iter().any(|x| x.starts_with("react:"));
            let Some(flow) = input.flows.flow(&f.id) else { continue };
            for n in &flow.nodes {
                let Some(e) = &n.expr else { continue };
                if react && n.concept == Concept::Var && n.writes.len() == 2 && e.concept == Concept::Call && e.label == "useState" {
                    let (state, setter) = (n.writes[0].clone(), n.writes[1].clone());
                    out.push(Candidate {
                        id: format!("state:hook:{}:{state}", f.id),
                        label: format!("{}: {state}", f.name),
                        detail: format!("React state changed with {setter}"),
                        score: 85.0,
                        spec: Spec::Hook { function: f.id.clone(), state, setter },
                    });
                }
            }
            // A local reassigned in a loop that switches on it.
            for n in flow.nodes.iter().filter(|n| n.concept == Concept::Var && n.writes.len() == 1) {
                let local = &n.writes[0];
                let switched = flow.nodes.iter().any(|m| matches!(m.concept, Concept::Match) && m.expr.as_ref().is_some_and(|e| e.concept == Concept::Var && e.label == *local));
                let reassigned = flow.nodes.iter().any(|m| m.concept == Concept::Assign && m.writes.iter().any(|w| w == local) && m.expr.as_ref().is_some_and(|e| e.label == "="));
                let looped = flow.nodes.iter().any(|m| matches!(m.concept, Concept::LoopWhile | Concept::LoopForEach | Concept::LoopRange | Concept::LoopInfinite));
                if switched && reassigned && looped {
                    out.push(Candidate {
                        id: format!("state:loop:{}:{local}", f.id),
                        label: format!("{}: {local}", f.name),
                        detail: "a local variable that drives a loop".into(),
                        score: 60.0,
                        spec: Spec::Loop { function: f.id.clone(), local: local.clone() },
                    });
                }
            }
        }
    }
    // Stored entities whose status is changed from outside (services, handlers).
    for m in &input.knowledge.models {
        if !scoped.types.contains(&m.symbol) || out.iter().any(|c| matches!(&c.spec, Spec::Entity { model, .. } if *model == m.symbol)) {
            continue;
        }
        if let Some(f) = m.model.fields.iter().find(|f| matches!(f.name.to_ascii_lowercase().as_str(), "status" | "state")) {
            let name = index.get(&m.symbol).map(|s| s.name.clone()).unwrap_or_default();
            out.push(Candidate {
                id: format!("state:entity:{}:{}", m.symbol, f.name),
                label: format!("{name} {} (stored)", f.name),
                detail: format!("life cycle of stored {name} records"),
                score: 88.0,
                spec: Spec::Entity { model: m.symbol.clone(), field: f.name.clone() },
            });
        }
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

fn implementations(index: &Index, iface: &str) -> Vec<String> {
    let mut out: Vec<String> = index
        .graph
        .edges
        .iter()
        .filter(|e| matches!(e.kind, StructEdgeKind::Implements | StructEdgeKind::Inherits) && e.to == iface)
        .filter(|e| index.get(&e.from).is_some_and(|s| !s.external && s.kind.is_type()))
        .map(|e| e.from.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// How a function is triggered, for transition labels: its route, its event, its name.
fn trigger_of(index: &Index, f: &Symbol) -> String {
    match index.info.get(f.id.as_str()).and_then(|i| i.entry.as_ref()) {
        Some(e) if matches!(e.kind, EntryKind::Route | EntryKind::Ui | EntryKind::Command | EntryKind::Message) => e.label.clone(),
        _ => bare(&f.name),
    }
}

fn read(index: &Index, input: &UmlInput, resolver: &Resolver, spec: &Spec) -> Machine {
    match spec {
        Spec::Declared(i) => declared(index, input, *i),
        Spec::Field { owner, field } => field_machine(index, input, resolver, owner, field),
        Spec::Entity { model, field } => entity_machine(index, input, resolver, model, field),
        Spec::Hook { function, state, setter } => hook_machine(index, input, function, state, setter),
        Spec::Pattern { owner, field, iface } => pattern_machine(index, input, owner, field, iface),
        Spec::Flags { owner, fields } => flags_machine(index, input, owner, fields),
        Spec::Loop { function, local } => loop_machine(index, input, resolver, function, local),
        Spec::TypeState { owner } => type_state(index, input, owner),
    }
}

fn declared(index: &Index, input: &UmlInput, i: usize) -> Machine {
    let f = &input.knowledge.state_machines[i];
    let mut m = Machine { title: format!("{} ({})", f.name, f.framework), detector: "declared", confidence: Some(Confidence::Exact), span: Some(f.span.clone()), owner: Some(f.owner.clone()), ..Machine::default() };
    m.states = f.states.clone();
    for t in &f.transitions {
        for s in [&t.from, &t.to] {
            if s != "*" && !m.states.contains(s) {
                m.states.push(s.clone());
            }
        }
        m.trans.push(Trans {
            from: (t.from != "*").then(|| t.from.clone()),
            to: t.to.clone(),
            trigger: t.trigger.clone(),
            guard: t.guard.as_deref().map(|g| g.trim().trim_start_matches("() =>").trim().to_owned()),
            confidence: Confidence::Exact,
            evidence: format!("declared with {}", f.framework),
            site: Some(f.span.clone()),
        });
    }
    if let Some(init) = &f.initial {
        m.initial.push(init.clone());
    }
    if !f.triggers.is_empty() {
        m.notes.push(format!("Also set from code: {}", f.triggers.join(", ")));
    }
    let _ = index;
    m
}

/// A field's initializer: in its signature (`= new Red()`) or, for a property, in the source.
fn initializer(input: &UmlInput, f: &Symbol) -> Option<String> {
    if let Some((_, v)) = f.signature.as_deref().and_then(|s| s.split_once(" = ")) {
        return value_of_text(v);
    }
    let o = f.origins.first()?;
    let src = input.flows.source(&o.file)?;
    let end = (o.end_byte as usize + 120).min(src.len());
    let text = src.get(o.start_byte as usize..end)?;
    let line = text.split_once("} =").map(|(_, r)| r).or_else(|| text.lines().next().and_then(|l| l.split_once(" = ").map(|(_, r)| r)))?;
    value_of_text(line.split([';', '\n']).next().unwrap_or(line))
}

fn enum_domain(index: &Index, resolver: &Resolver, near: &Symbol, type_text: &str) -> Option<(Vec<String>, HashMap<String, String>)> {
    let shape = analyze(type_text)?;
    let e = resolver.type_named(&shape.target, near).filter(|e| e.kind == SymbolKind::Enum)?;
    let members: Vec<&Symbol> = index.children.get(e.id.as_str()).into_iter().flatten().copied().filter(|m| m.kind == SymbolKind::EnumMember).collect();
    let mut sorted = members.clone();
    sorted.sort_by_key(|m| m.origins.first().map(|o| o.start_byte).unwrap_or(0));
    Some((sorted.iter().map(|m| m.name.clone()).collect(), sorted.iter().map(|m| (m.name.clone(), m.id.clone())).collect()))
}

fn literal_domain(type_text: &str) -> Vec<String> {
    type_text.split('|').filter_map(|p| p.trim().strip_prefix('"').and_then(|p| p.strip_suffix('"')).map(str::to_owned)).collect()
}

/// Turns walker events into the machine's transitions.
fn absorb(m: &mut Machine, index: &Index, w: &Walker, v: usize, evidence: &dyn Fn(&engine::Event) -> String) {
    let domain: BTreeSet<String> = w.vars[v].domain.iter().cloned().collect();
    for e in w.events.iter().filter(|e| e.var == v) {
        let froms: Vec<Option<String>> = match &e.from {
            Some(set) if *set != domain || domain.len() <= 1 => set.iter().cloned().map(Some).collect(),
            _ => vec![None],
        };
        for from in froms {
            if from.as_deref() == Some(e.to.as_str()) {
                continue;
            }
            m.trans.push(Trans {
                from,
                to: e.to.clone(),
                trigger: (!e.trigger.is_empty()).then(|| e.trigger.clone()),
                guard: e.guard.clone(),
                confidence: Confidence::Inferred,
                evidence: evidence(e),
                site: Some(e.site.clone()),
            });
        }
    }
    let _ = index;
}

fn methods_of<'a>(index: &'a Index, owner: &str) -> Vec<&'a Symbol> {
    index.children.get(owner).into_iter().flatten().copied().filter(|m| m.has_flow && matches!(m.kind, SymbolKind::Method | SymbolKind::Function | SymbolKind::Property)).collect()
}

fn constructors<'a>(index: &'a Index, owner: &Symbol) -> Vec<&'a Symbol> {
    index
        .children
        .get(owner.id.as_str())
        .into_iter()
        .flatten()
        .copied()
        .filter(|m| m.has_flow && (m.kind == SymbolKind::Constructor || (m.language == Language::Rust && !m.flavors.iter().any(|f| f.contains("self")) && m.type_text.as_deref().is_some_and(|t| t == "Self" || t == owner.name))))
        .collect()
}

/// Values a constructor gives the field (`Status = Draft`, `Player { state: PlayerState::Idle }`).
fn constructor_values(index: &Index, input: &UmlInput, owner: &Symbol, var: &Var) -> Vec<String> {
    let mut out = Vec::new();
    for c in constructors(index, owner) {
        let mut w = Walker::new(index, input.flows, vec![var.clone()]);
        w.function(c, Some(owner), "");
        out.extend(w.events.iter().map(|e| e.to.clone()));
        // Struct literals: `state: PlayerState::Idle`.
        if let (Some(flow), Matcher::Field(field)) = (input.flows.flow(&c.id), &var.matcher) {
            fn find(e: &Expr, field: &str, out: &mut Vec<String>) {
                if let Some(rest) = e.label.strip_prefix(&format!("{field}: "))
                    && let Some(v) = value_of_text(rest)
                {
                    out.push(v);
                }
                for c in &e.children {
                    find(c, field, out);
                }
            }
            for n in &flow.nodes {
                if let Some(e) = &n.expr {
                    find(e, field, &mut out);
                }
            }
        }
    }
    out.retain(|v| var.open || var.domain.contains(v));
    out.dedup();
    out
}

fn field_machine(index: &Index, input: &UmlInput, resolver: &Resolver, owner: &str, field: &str) -> Machine {
    let Some(t) = index.get(owner) else { return Machine::default() };
    let Some(f) = index.children.get(owner).into_iter().flatten().find(|m| m.name == field).copied() else { return Machine::default() };
    let ty = f.type_text.clone().unwrap_or_default();
    let mut m = Machine { title: format!("{}.{}", t.name, f.name), detector: "field", span: f.origins.first().cloned(), owner: Some(t.id.clone()), ..Machine::default() };
    let (domain, open) = match enum_domain(index, resolver, t, &ty) {
        Some((d, syms)) => {
            m.symbols = syms;
            m.detector = "enum";
            (d, false)
        }
        None if ty == "bool" || ty == "boolean" => (vec!["false".into(), "true".into()], false),
        None => {
            let lits = literal_domain(&ty);
            let open = lits.is_empty();
            (lits, open)
        }
    };
    let var = Var { matcher: Matcher::Field(f.name.clone()), domain, open };
    let mut w = Walker::new(index, input.flows, vec![var.clone()]);
    for pass in 0..2 {
        if pass == 1 && !w.second_pass() {
            break;
        }
        for meth in methods_of(index, owner) {
            w.function(meth, Some(t), &trigger_of(index, meth));
        }
    }
    // TypeScript and Rust write fields through `this`/`self`; C# often without.
    absorb(&mut m, index, &w, 0, &|e| format!("assigned in {}", index.get(&e.method).map(|s| bare(&s.name)).unwrap_or_default()));
    m.states = w.vars[0].domain.clone();
    let mut init: Vec<String> = initializer(input, f).into_iter().filter(|v| m.states.contains(v)).collect();
    if init.is_empty() {
        init = constructor_values(index, input, t, &w.vars[0]);
    }
    if init.is_empty() && t.language == Language::Csharp && !open && !m.states.is_empty() {
        // C# fields start at the type's default: the first enum member, `false`.
        init.push(m.states[0].clone());
        m.notes.push(format!("{} starts at its default value", f.name));
    }
    m.initial = init;
    m.confidence = Some(confidence(&m, w.reads));
    m
}

fn confidence(m: &Machine, reads: usize) -> Confidence {
    let known = m.trans.iter().any(|t| t.from.is_some());
    if m.states.len() >= 2 && known && reads > 0 { Confidence::Inferred } else if m.trans.is_empty() { Confidence::Unresolved } else { Confidence::Inferred }
}

fn entity_machine(index: &Index, input: &UmlInput, resolver: &Resolver, model: &str, field: &str) -> Machine {
    let Some(t) = index.get(model) else { return Machine::default() };
    let f = index.children.get(model).into_iter().flatten().find(|m| m.name == field).copied();
    let ty = f.and_then(|f| f.type_text.clone()).or_else(|| input.knowledge.models.iter().find(|m| m.symbol == model).and_then(|m| m.model.fields.iter().find(|x| x.name == field)).and_then(|x| x.ty.clone())).unwrap_or_default();
    let mut m = Machine { title: format!("{} {} (stored records)", t.name, field), detector: "entity", span: t.origins.first().cloned(), owner: Some(t.id.clone()), ..Machine::default() };
    let (domain, open) = match enum_domain(index, resolver, t, &ty) {
        Some((d, syms)) => {
            m.symbols = syms;
            (d, false)
        }
        None => (Vec::new(), true),
    };
    // Variables that hold a record: named after the type, or declared with it.
    let lower = t.name.to_ascii_lowercase();
    let mut receivers = vec![lower.clone(), "entity".to_owned(), "record".to_owned(), format!("existing{lower}"), format!("{lower}entity")];
    let candidates: Vec<&Symbol> = index
        .graph
        .symbols
        .iter()
        .filter(|s| s.has_flow && !s.external && !index.is_test(&s.id))
        .filter(|s| {
            let caps = index.info.get(s.id.as_str()).map(|i| i.capabilities.clone()).unwrap_or_default();
            caps.iter().any(|c| c.starts_with("DB_")) || s.signature.as_deref().is_some_and(|sig| sig.contains(&t.name)) || index.owner_type(&s.id).is_some_and(|o| o.id == model)
        })
        .collect();
    for s in &candidates {
        if let Some(sig) = &s.signature {
            for (name, ty) in crate::types::parameter_types(sig, &s.name, s.language == Language::Csharp) {
                if analyze(&ty).is_some_and(|x| x.target == t.name) {
                    receivers.push(name);
                }
            }
        }
    }
    receivers.sort();
    receivers.dedup();
    let var = Var { matcher: Matcher::Member { field: field.to_owned(), receivers }, domain: domain.clone(), open };
    let own = Var { matcher: Matcher::Field(field.to_owned()), domain, open };
    let mut w = Walker::new(index, input.flows, vec![var, own]);
    let routes = &input.knowledge.routes;
    let mut jobs = Vec::new();
    for s in candidates {
        // Triggered by the route whose handler calls it (or is it).
        let mut trigger = trigger_of(index, s);
        if !routes.iter().any(|r| r.handler == s.id) {
            for r in routes {
                let Some(flow) = input.flows.flow(&r.handler) else { continue };
                if flow.nodes.iter().any(|n| call_sites(n).iter().any(|c| c.name == bare(&s.name))) {
                    trigger = format!("{} {}", r.method, r.path);
                    break;
                }
            }
        }
        let owner = index.owner_type(&s.id).filter(|o| o.id != s.id);
        jobs.push((s, owner, trigger));
    }
    for pass in 0..2 {
        if pass == 1 && !w.second_pass() {
            break;
        }
        for (s, owner, trigger) in &jobs {
            w.function(s, *owner, trigger);
        }
    }
    // Both ways of writing the field feed one machine.
    let mut merged = w.vars[0].domain.clone();
    for v in &w.vars[1].domain {
        if !merged.contains(v) {
            merged.push(v.clone());
        }
    }
    let ev = |e: &engine::Event| format!("{} sets {field}", index.get(&e.method).map(|s| bare(&s.name)).unwrap_or_default());
    absorb(&mut m, index, &w, 0, &ev);
    absorb(&mut m, index, &w, 1, &ev);
    m.states = merged;
    if let Some(f) = f {
        m.initial = initializer(input, f).into_iter().filter(|v| m.states.contains(v)).collect();
    }
    if m.initial.is_empty() && t.language == Language::Csharp && !m.states.is_empty() && !w.vars[0].open {
        m.initial.push(m.states[0].clone());
        m.notes.push(format!("new records start at the default {field}"));
    }
    m.confidence = Some(confidence(&m, w.reads));
    m
}

fn hook_machine(index: &Index, input: &UmlInput, function: &str, state: &str, setter: &str) -> Machine {
    let Some(f) = index.get(function) else { return Machine::default() };
    let mut m = Machine { title: format!("{}: {state}", f.name), detector: "react", span: f.origins.first().cloned(), owner: Some(f.id.clone()), ..Machine::default() };
    let var = Var { matcher: Matcher::Setter { state: state.to_owned(), setter: setter.to_owned() }, domain: Vec::new(), open: true };
    let mut w = Walker::new(index, input.flows, vec![var]);
    // `useState(initial)`.
    if let Some(flow) = input.flows.flow(function)
        && let Some(n) = flow.nodes.iter().find(|n| n.writes.iter().any(|x| x == setter))
        && let Some(arg) = n.expr.as_ref().and_then(|e| e.children.first())
        && let Some(v) = literal_value(arg, &HashMap::new())
    {
        w.vars[0].domain.push(v.clone());
        m.initial.push(v);
    }
    // The component (on render) and its handlers.
    let mut fns = vec![f];
    let mut stack = vec![f.id.as_str()];
    while let Some(id) = stack.pop() {
        for c in index.children.get(id).into_iter().flatten().filter(|c| c.has_flow) {
            fns.push(c);
            stack.push(c.id.as_str());
        }
    }
    for pass in 0..2 {
        if pass == 1 && !w.second_pass() {
            break;
        }
        for x in &fns {
            let trigger = if x.id == f.id { "render".to_owned() } else { trigger_of(index, x) };
            w.function(x, None, &trigger);
        }
    }
    absorb(&mut m, index, &w, 0, &|e| format!("{setter}(…) in {}", index.get(&e.method).map(|s| bare(&s.name)).unwrap_or_default()));
    m.states = w.vars[0].domain.clone();
    m.confidence = Some(if m.states.len() >= 2 { Confidence::Inferred } else { Confidence::Unresolved });
    m
}

fn pattern_machine(index: &Index, input: &UmlInput, owner: &str, field: &str, iface: &str) -> Machine {
    let Some(ctx) = index.get(owner) else { return Machine::default() };
    let impls = implementations(index, iface);
    let names: HashMap<String, String> = impls.iter().filter_map(|i| index.get(i).map(|s| (s.name.clone(), s.id.clone()))).collect();
    let mut m = Machine { title: format!("{}.{} (State pattern)", ctx.name, field), detector: "pattern", owner: Some(ctx.id.clone()), ..Machine::default() };
    let mut order: Vec<String> = impls.iter().filter_map(|i| index.get(i).map(|s| s.name.clone())).collect();
    m.symbols = names.clone();
    // Which context methods delegate to which state method (`Tick` → `_state.Next(this)`).
    let mut delegates: HashMap<String, String> = HashMap::new();
    for meth in methods_of(index, owner) {
        let Some(flow) = input.flows.flow(&meth.id) else { continue };
        for n in &flow.nodes {
            for c in call_sites(n) {
                if c.receiver.is_some_and(|r| r.label.trim_start_matches('.') == field || r.label == format!("this.{field}")) {
                    delegates.insert(c.name.clone(), bare(&meth.name));
                }
            }
        }
    }
    let mut sources: Vec<(Option<String>, &Symbol)> = impls.iter().filter_map(|i| index.get(i)).map(|s| (Some(s.name.clone()), s)).collect();
    sources.push((None, ctx));
    for (from, holder) in sources {
        for meth in methods_of(index, &holder.id) {
            let Some(flow) = input.flows.flow(&meth.id) else { continue };
            for n in &flow.nodes {
                for c in call_sites(n).iter().filter(|c| c.construct && names.contains_key(&c.name)) {
                    let name = bare(&meth.name);
                    let trigger = delegates.get(&name).map(|d| if *d == name { name.clone() } else { format!("{d} ({name})") }).unwrap_or(name);
                    m.trans.push(Trans {
                        from: from.clone(),
                        to: c.name.clone(),
                        trigger: Some(trigger),
                        guard: None,
                        confidence: Confidence::Inferred,
                        evidence: format!("{} creates the next state {}", holder.name, c.name),
                        site: Some(c.expr.span.clone()),
                    });
                }
            }
        }
    }
    // Creations in the context's own field initializer are the start, not transitions.
    if let Some(f) = index.children.get(owner).into_iter().flatten().find(|x| x.name == field)
        && let Some(v) = initializer(input, f)
    {
        m.initial.push(v);
    }
    order.sort_by_key(|n| (!m.initial.contains(n), n.clone()));
    m.states = order;
    m.confidence = Some(Confidence::Inferred);
    m
}

fn flags_machine(index: &Index, input: &UmlInput, owner: &str, fields: &[String]) -> Machine {
    let Some(t) = index.get(owner) else { return Machine::default() };
    let vars: Vec<Var> = fields.iter().map(|f| Var { matcher: Matcher::Field(f.clone()), domain: vec!["false".into(), "true".into()], open: false }).collect();
    let mut w = Walker::new(index, input.flows, vars);
    // Each method: the ways it ends normally, as (required values, assigned values).
    let mut rules: Vec<(String, Path)> = Vec::new();
    for meth in methods_of(index, owner) {
        let trigger = bare(&meth.name);
        for p in w.function(meth, Some(t), &trigger).paths {
            if p.cur.iter().any(|c| c.is_some()) {
                rules.push((trigger.clone(), p));
            }
        }
    }
    let label = |s: &[bool]| -> String {
        let on: Vec<String> = fields.iter().zip(s).filter(|(_, v)| **v).map(|(f, _)| f.trim_start_matches('_').to_owned()).collect();
        if on.is_empty() {
            if fields.len() == 1 { format!("not {}", fields[0].trim_start_matches('_')) } else { "none set".to_owned() }
        } else {
            on.join(" + ")
        }
    };
    let start: Vec<bool> = fields
        .iter()
        .map(|f| index.children.get(owner).into_iter().flatten().find(|x| x.name == *f).and_then(|x| initializer(input, x)).is_some_and(|v| v == "true"))
        .collect();
    let mut m = Machine { title: format!("{} flags", t.name), detector: "flags", owner: Some(t.id.clone()), span: t.origins.first().cloned(), ..Machine::default() };
    let mut seen = vec![start.clone()];
    let mut queue = vec![start.clone()];
    m.initial.push(label(&start));
    while let Some(s) = queue.pop() {
        for (trigger, p) in &rules {
            let admits = (0..fields.len()).all(|v| p.req[v].as_ref().is_none_or(|r| r.contains(if s[v] { "true" } else { "false" })));
            if !admits {
                continue;
            }
            let mut next = s.clone();
            for (slot, cur) in next.iter_mut().zip(&p.cur) {
                if let Some(Some(x)) = cur {
                    *slot = x == "true";
                }
            }
            if next != s {
                m.trans.push(Trans {
                    from: Some(label(&s)),
                    to: label(&next),
                    trigger: Some(trigger.clone()),
                    guard: (!p.guards.is_empty()).then(|| p.guards.join(" and ")),
                    confidence: Confidence::Inferred,
                    evidence: format!("{trigger} sets {}", fields.iter().zip(&p.cur).filter_map(|(f, c)| c.clone().flatten().map(|x| format!("{f} = {x}"))).collect::<Vec<_>>().join(", ")),
                    site: None,
                });
            }
            if !seen.contains(&next) {
                seen.push(next.clone());
                queue.push(next);
            }
        }
    }
    m.states = seen.iter().map(|s| label(s)).collect();
    m.trans.sort_by(|a, b| (&a.from, &a.to, &a.trigger).cmp(&(&b.from, &b.to, &b.trigger)));
    m.trans.dedup_by(|a, b| a.from == b.from && a.to == b.to && a.trigger == b.trigger);
    m.confidence = Some(if m.states.len() >= 2 { Confidence::Inferred } else { Confidence::Unresolved });
    m
}

fn find_loop<'b>(block: &'b Block<'b>, local: &str) -> Option<&'b Block<'b>> {
    for s in &block.steps {
        let is_loop = matches!(s.node.concept, Concept::LoopWhile | Concept::LoopForEach | Concept::LoopRange | Concept::LoopInfinite);
        for a in &s.arms {
            if is_loop && a.pin == "body" && mentions(&a.block, local) {
                return Some(&a.block);
            }
            if let Some(b) = find_loop(&a.block, local) {
                return Some(b);
            }
        }
    }
    None
}

fn mentions(block: &Block, local: &str) -> bool {
    block.steps.iter().any(|s| s.node.writes.iter().any(|w| w == local) || s.node.expr.as_ref().is_some_and(|e| e.concept == Concept::Var && e.label == local) || s.arms.iter().any(|a| mentions(&a.block, local)))
}

fn loop_machine(index: &Index, input: &UmlInput, resolver: &Resolver, function: &str, local: &str) -> Machine {
    let Some(f) = index.get(function) else { return Machine::default() };
    let Some(flow) = input.flows.flow(function) else { return Machine::default() };
    let mut m = Machine { title: format!("{}: {local}", bare(&f.name)), detector: "loop", owner: Some(f.id.clone()), span: f.origins.first().cloned(), ..Machine::default() };
    let decl = flow.nodes.iter().find(|n| n.concept == Concept::Var && n.writes.iter().any(|w| w == local));
    let init = decl.and_then(|n| n.expr.as_ref()).and_then(|e| literal_value(e, &HashMap::new()));
    // `Mode.Start`: the enum named before the dot gives the states.
    let enum_name = decl.and_then(|n| n.expr.as_ref()).filter(|e| e.concept == Concept::GetMember).and_then(|e| e.children.first()).map(|c| c.label.clone());
    let (domain, open) = match enum_name.as_deref().and_then(|n| enum_domain(index, resolver, f, n)) {
        Some((d, syms)) => {
            m.symbols = syms;
            (d, false)
        }
        None => (init.clone().into_iter().collect(), true),
    };
    let var = Var { matcher: Matcher::Local(local.to_owned()), domain, open };
    let mut w = Walker::new(index, input.flows, vec![var]);
    let body = tree(&flow);
    if let Some(lp) = find_loop(&body, local) {
        w.block_of(f, &flow, lp, vec![Path::new(1)], "");
        if w.second_pass() {
            w.block_of(f, &flow, lp, vec![Path::new(1)], "");
        }
    }
    absorb(&mut m, index, &w, 0, &|_| format!("`{local}` is reassigned in the loop"));
    m.states = w.vars[0].domain.clone();
    m.initial = init.into_iter().filter(|v| m.states.contains(v)).collect();
    m.confidence = Some(confidence(&m, w.reads));
    m
}

/// Rust type-state: the `impl Door<Closed>` around a method gives its source state.
fn type_state(index: &Index, input: &UmlInput, owner: &str) -> Machine {
    let Some(t) = index.get(owner) else { return Machine::default() };
    let mut m = Machine { title: format!("{} (type-state)", t.name), detector: "type-state", owner: Some(t.id.clone()), span: t.origins.first().cloned(), ..Machine::default() };
    let arg = |text: &str| -> Option<String> {
        let inner = text.split_once(&format!("{}<", t.name))?.1;
        Some(inner.split(['>', ',']).next()?.trim().to_owned())
    };
    for meth in index.children.get(owner).into_iter().flatten().filter(|m| matches!(m.kind, SymbolKind::Method | SymbolKind::Function)) {
        let Some(to) = meth.type_text.as_deref().and_then(arg) else { continue };
        let from = meth.origins.first().and_then(|o| {
            let src = input.flows.source(&o.file)?;
            let before = src.get(..o.start_byte as usize)?;
            let header = before.rfind("impl")?;
            arg(before[header..].lines().next()?)
        });
        let consumes = meth.flavors.iter().any(|f| f == "self");
        for s in [from.clone(), Some(to.clone())].into_iter().flatten() {
            if !m.states.contains(&s) && s.chars().next().is_some_and(char::is_uppercase) {
                m.states.push(s);
            }
        }
        match (consumes, from) {
            (true, Some(from)) if from != to => m.trans.push(Trans {
                from: Some(from),
                to,
                trigger: Some(bare(&meth.name)),
                guard: None,
                confidence: Confidence::Inferred,
                evidence: format!("{} takes `self` and returns {}", bare(&meth.name), meth.type_text.clone().unwrap_or_default()),
                site: meth.origins.first().cloned(),
            }),
            (false, _) => m.initial.push(to),
            _ => {}
        }
    }
    for s in &m.states {
        if let Some(sym) = index.graph.symbols.iter().find(|x| x.name == *s && x.kind.is_type() && x.language == Language::Rust) {
            m.symbols.insert(s.clone(), sym.id.clone());
        }
    }
    m.confidence = Some(Confidence::Inferred);
    m
}

/// Text of a transition label: no characters the state diagram grammar reads.
fn text(t: &str, max: usize) -> String {
    shorten(t, max)
        .chars()
        .filter(|c| !matches!(c, ';' | '#' | '"'))
        .map(|c| match c {
            ':' => '∶',
            '{' => '‹',
            '}' => '›',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn render(d: &mut UmlDiagram, index: &Index, m: &Machine, opts: &UmlOptions) {
    let mut m_trans: Vec<Trans> = m.trans.iter().filter(|t| opts.allows(Some(t.confidence))).cloned().collect();
    // Same transition read twice (two paths): once.
    let mut seen = BTreeSet::new();
    m_trans.retain(|t| seen.insert((t.from.clone(), t.to.clone(), t.trigger.clone(), t.guard.clone())));
    let mut states = m.states.clone();
    for t in &m_trans {
        for s in [t.from.as_ref(), Some(&t.to)].into_iter().flatten() {
            if !states.contains(s) {
                states.push(s.clone());
            }
        }
    }
    let ids: HashMap<&str, String> = states.iter().enumerate().map(|(i, s)| (s.as_str(), format!("n{i}"))).collect();
    // Reachable from the start (a nested state with its parent); without a known start, any
    // state something moves to.
    let mut reached: BTreeSet<&str> = m.initial.iter().map(String::as_str).collect();
    if reached.is_empty() {
        reached.extend(m_trans.iter().map(|t| t.to.as_str()));
        reached.extend(m_trans.iter().filter_map(|t| t.from.as_deref()));
    }
    loop {
        let before = reached.len();
        let parents: Vec<&str> = states.iter().filter(|s| reached.contains(s.as_str())).map(String::as_str).collect();
        for s in &states {
            let nested_in_reached = s.rsplit_once('.').is_some_and(|(p, _)| parents.contains(&p));
            let parent_of_reached = parents.iter().any(|r| r.rsplit_once('.').is_some_and(|(p, _)| p == s));
            if nested_in_reached || parent_of_reached {
                reached.insert(s.as_str());
            }
        }
        for t in &m_trans {
            if t.from.as_deref().is_none_or(|f| reached.contains(f)) {
                reached.insert(t.to.as_str());
            }
        }
        if reached.len() == before {
            break;
        }
    }
    let reached: BTreeSet<String> = reached.into_iter().map(str::to_owned).collect();
    // A state nothing reaches cannot be left either.
    m_trans.retain(|t| t.from.as_deref().is_none_or(|f| reached.contains(f)));
    let leaves: BTreeSet<&str> = m_trans.iter().filter(|t| t.from.as_deref() != Some(t.to.as_str())).filter_map(|t| t.from.as_deref()).collect();
    let any_leaves = m_trans.iter().any(|t| t.from.is_none());
    let unreachable: Vec<&String> = states.iter().filter(|s| !reached.contains(s.as_str()) && !s.contains('.')).collect();
    // Final: nothing leaves it and its name or the method that sets it says it ends.
    let mut finals: BTreeSet<String> = m.finals.clone();
    for s in &states {
        if leaves.contains(s.as_str()) || !reached.contains(s.as_str()) || m.initial.contains(s) {
            continue;
        }
        let low = s.to_ascii_lowercase();
        let by_name = TERMINAL.iter().any(|w| low == *w || low.ends_with(w));
        let by_method = m_trans.iter().filter(|t| t.to == *s).any(|t| t.trigger.as_deref().is_some_and(|tr| TERMINAL_METHODS.iter().any(|w| tr.to_ascii_lowercase().starts_with(w))));
        // A transition from any state would leave it, unless it leads to this very state.
        let kept = !any_leaves || m_trans.iter().filter(|t| t.from.is_none()).all(|t| t.to == *s);
        if (by_name || by_method) && kept {
            finals.insert(s.clone());
        }
    }

    let mut out = String::from("stateDiagram-v2\n");
    let mut classes: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    // Nested states (`success.fresh`) inside their parent.
    fn declare(out: &mut String, s: &str, ids: &HashMap<&str, String>, depth: usize, states: &[String]) {
        let pad = "  ".repeat(depth);
        let id = &ids[s];
        let name = s.rsplit('.').next().unwrap_or(s);
        out.push_str(&format!("{pad}state \"{}\" as {id}\n", text(name, 40)));
        let kids: Vec<&String> = states.iter().filter(|x| x.rsplit_once('.').is_some_and(|(parent, _)| parent == s)).collect();
        if !kids.is_empty() {
            out.push_str(&format!("{pad}state {id} {{\n"));
            for k in kids {
                declare(out, k, ids, depth + 1, states);
            }
            out.push_str(&format!("{pad}}}\n"));
        }
    }
    for s in states.iter().filter(|s| !s.contains('.') || !states.contains(&s.rsplit_once('.').map(|(p, _)| p.to_owned()).unwrap_or_default())) {
        declare(&mut out, s, &ids, 1, &states);
    }
    for i in &m.initial {
        if let Some(id) = ids.get(i.as_str()) {
            out.push_str(&format!("  [*] --> {id}\n"));
        }
    }
    let any_id = "nAny".to_owned();
    let mut relations = Vec::new();
    let mut used_any = false;
    for t in &m_trans {
        let label = match (&t.trigger, &t.guard) {
            (Some(tr), Some(g)) => format!("{} [{}]", text(tr, 40), text(g, 50)),
            (Some(tr), None) => text(tr, 40),
            (None, Some(g)) => format!("[{}]", text(g, 50)),
            (None, None) => String::new(),
        };
        let to = &ids[t.to.as_str()];
        let froms: Vec<String> = match &t.from {
            Some(f) => vec![ids[f.as_str()].clone()],
            // From any state: drawn from each one when there are few, else from one "any state" box.
            None if states.len() <= 6 => states.iter().filter(|s| **s != t.to && !s.contains('.') && reached.contains(s.as_str())).map(|s| ids[s.as_str()].clone()).collect(),
            None => {
                used_any = true;
                vec![any_id.clone()]
            }
        };
        for f in froms {
            if label.is_empty() {
                out.push_str(&format!("  {f} --> {to}\n"));
            } else {
                out.push_str(&format!("  {f} --> {to} : {label}\n"));
            }
            relations.push(UmlRelation {
                from: f,
                to: to.clone(),
                kind: "transition".into(),
                label: (!label.is_empty()).then(|| label.clone()),
                confidence: (t.confidence != Confidence::Exact).then_some(t.confidence),
                evidence: Some(t.evidence.clone()),
                site: t.site.clone(),
                count: 1,
            });
        }
    }
    if used_any {
        out.insert_str("stateDiagram-v2\n".len(), &format!("  state \"any state\" as {any_id}\n"));
        classes.entry("ctx").or_default().push(any_id.clone());
    }
    for f in &finals {
        out.push_str(&format!("  {} --> [*]\n", ids[f.as_str()]));
    }
    for s in &unreachable {
        let id = &ids[s.as_str()];
        out.push_str(&format!("  note right of {id} : never set\n"));
        classes.entry("ctx").or_default().push(id.clone());
    }
    let used: Vec<&str> = classes.keys().copied().collect();
    class_defs(&mut out, &used);
    for (c, list) in &classes {
        out.push_str(&format!("  class {} {c}\n", list.join(",")));
    }
    d.mermaid = out;
    d.title = format!("States of {}", m.title);
    d.elements = states
        .iter()
        .map(|s| {
            let sym = m.symbols.get(s).cloned().or_else(|| m.owner.clone());
            UmlElement {
                mermaid_id: ids[s.as_str()].clone(),
                span: m.symbols.get(s).and_then(|x| index.get(x)).and_then(|x| x.origins.first().cloned()).or_else(|| m.span.clone()),
                symbol: sym,
                label: s.rsplit('.').next().unwrap_or(s).to_owned(),
                kind: if finals.contains(s) { "final state".into() } else if m.initial.contains(s) { "initial state".into() } else { "state".into() },
                context: unreachable.contains(&s),
                detail: Some(format!("detected from {}", m.detector)),
            }
        })
        .collect();
    d.relations = relations;
    if let Some(c) = m.confidence
        && c != Confidence::Exact
    {
        d.warnings.push(format!(
            "Read from the code ({} detector): transitions come from assignments and the conditions around them; check the guards.",
            m.detector
        ));
    }
    if !unreachable.is_empty() {
        d.warnings.push(format!("Never set by the code: {}.", unreachable.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
    for n in &m.notes {
        d.warnings.push(n.clone());
    }
    let stuck: Vec<&String> = states.iter().filter(|s| reached.contains(s.as_str()) && !leaves.contains(s.as_str()) && !finals.contains(*s) && !any_leaves && !s.contains('.')).collect();
    if !stuck.is_empty() && !m_trans.is_empty() {
        d.warnings.push(format!("No way out of: {} (final, or a missing transition?).", stuck.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
}
