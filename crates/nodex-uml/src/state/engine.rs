//! Path-sensitive reading of how functions change state variables.
//!
//! A function's flow is walked as nested blocks, carrying a set of *paths*. Each path knows,
//! for every tracked variable, which values it may hold when the function was entered (narrowed
//! by the conditions taken on the way: `if (Status != Draft) throw` leaves only `Draft` after
//! the guard; `case Paid:` only `Paid`; a helper `EnsureStatus(Submitted)` is followed into its
//! body) and which value the function has assigned since. An assignment of a known value is a
//! transition from the values the path allows to the new one; conditions that do not involve a
//! tracked variable become the transition's guard.

use std::collections::{BTreeSet, HashMap};

use nodex_ir::{Concept, Expr, FlowGraph, FlowNode, Language, Span, Symbol, SymbolKind};

use crate::FlowSource;
use crate::flowtree::{Block, call_sites, tree};
use crate::index::Index;
use crate::types::parameter_types;

pub type Set = BTreeSet<String>;

const MAX_PATHS: usize = 64;

/// How a tracked variable appears in code.
#[derive(Clone, Debug, PartialEq)]
pub enum Matcher {
    /// A field of the type being read: `Status`, `this.status`, `self.state`.
    Field(String),
    /// A local variable.
    Local(String),
    /// A field reached through a variable of a known type: `order.Status`.
    Member { field: String, receivers: Vec<String> },
    /// React `const [status, setStatus] = useState(...)`: read as `status`, written with `setStatus(x)`.
    Setter { state: String, setter: String },
}

impl Matcher {
    pub fn reads(&self, e: &Expr) -> bool {
        let is_self = |c: &Expr| c.concept == Concept::Var && matches!(c.label.as_str(), "this" | "self");
        match self {
            Matcher::Field(n) => match e.concept {
                Concept::Var => e.label == *n,
                Concept::GetMember => e.label.trim_start_matches('.') == n && e.children.first().is_none_or(is_self),
                _ => false,
            },
            Matcher::Local(n) => e.concept == Concept::Var && e.label == *n,
            Matcher::Member { field, receivers } => {
                e.concept == Concept::GetMember
                    && e.label.trim_start_matches('.') == field
                    && e.children.first().is_some_and(|c| c.concept == Concept::Var && receivers.iter().any(|r| r.eq_ignore_ascii_case(&c.label)))
            }
            Matcher::Setter { state, .. } => e.concept == Concept::Var && e.label == *state,
        }
    }
}

/// A variable whose values are the states.
#[derive(Clone, Debug)]
pub struct Var {
    pub matcher: Matcher,
    /// Known values, in declaration order (enum members) or first-seen order (literals).
    pub domain: Vec<String>,
    /// Values outside `domain` are accepted and added (string literals).
    pub open: bool,
}

/// The value an expression denotes: `OrderStatus.Paid` → `Paid`, `"idle"` → `idle`,
/// `PlayerState::Idle` → `Idle`, `new Green()` → `Green`, `true`.
pub fn literal_value(e: &Expr, bindings: &HashMap<String, String>) -> Option<String> {
    match e.concept {
        Concept::Literal => {
            let t = e.label.rsplit_once(": ").map(|(_, v)| v).unwrap_or(&e.label).trim();
            let unq = t.trim_matches(|c| c == '"' || c == '\'' || c == '`');
            (!unq.is_empty() && !unq.contains('{')).then(|| unq.to_owned())
        }
        Concept::GetMember => {
            let last = e.label.rsplit('.').next().unwrap_or("").trim();
            (!last.is_empty() && last.chars().all(|c| c.is_alphanumeric() || c == '_')).then(|| last.to_owned())
        }
        Concept::Var => Some(bindings.get(&e.label).cloned().unwrap_or_else(|| e.label.rsplit("::").next().unwrap_or(&e.label).to_owned())),
        Concept::Construct => Some(e.label.trim_start_matches("new ").split("::").next().unwrap_or("").trim().to_owned()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// Values a `case` label names (`OrderStatus.Paid`, `PlayerState::Idle | PlayerState::Running`,
/// `"done"`), or `None` for a default case.
pub fn case_values(label: &str) -> Option<Vec<String>> {
    let l = label.trim().trim_start_matches("case ").trim_end_matches(':').trim_end_matches("=>").trim();
    if matches!(l, "_" | "default" | "else" | "") || l.starts_with("default") {
        return None;
    }
    Some(
        l.split(['|', ','])
            .map(|p| {
                let p = p.trim().split(" when ").next().unwrap_or("").split(" if ").next().unwrap_or("").trim();
                let p = p.split('(').next().unwrap_or(p);
                p.rsplit(['.', ':']).next().unwrap_or(p).trim_matches(|c| c == '"' || c == '\'' || c == '`').trim().to_owned()
            })
            .filter(|p| !p.is_empty())
            .collect(),
    )
}

/// The opposite of a condition, written plainly (`amount < Total` → `amount >= Total`).
pub fn negate(text: &str) -> String {
    let t = text.trim();
    if let Some(inner) = t.strip_prefix('!').filter(|i| !i.contains(' ')) {
        return inner.to_owned();
    }
    for (op, neg) in [(" <= ", " > "), (" >= ", " < "), (" ≤ ", " > "), (" ≥ ", " < "), (" == ", " != "), (" === ", " !== "), (" != ", " == "), (" ≠ ", " == "), (" !== ", " === "), (" < ", " >= "), (" > ", " <= ")] {
        if t.matches(op).count() == 1 && !t.contains("&&") && !t.contains("||") && !t.contains(" and ") && !t.contains(" or ") {
            return t.replacen(op, neg, 1);
        }
    }
    format!("not ({t})")
}

/// A change of state seen in code.
#[derive(Clone, Debug)]
pub struct Event {
    pub var: usize,
    /// Values the variable may hold before (`None`: any).
    pub from: Option<Set>,
    pub to: String,
    pub trigger: String,
    pub guard: Option<String>,
    /// The function the assignment is in.
    pub method: String,
    pub site: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    /// Values each variable may have had on entry (`None`: unconstrained).
    pub req: Vec<Option<Set>>,
    /// What the function assigned: `None` untouched, `Some(None)` an unknown value.
    pub cur: Vec<Option<Option<String>>>,
    pub guards: Vec<String>,
}

impl Path {
    pub fn new(vars: usize) -> Self {
        Self { req: vec![None; vars], cur: vec![None; vars], guards: Vec::new() }
    }

    /// Keeps the path only when the variable can hold one of `set`.
    fn narrow(&mut self, v: usize, set: &Set) -> bool {
        match &self.cur[v] {
            Some(Some(x)) => set.contains(x),
            Some(None) => true,
            None => {
                let next: Set = match &self.req[v] {
                    Some(r) => r.intersection(set).cloned().collect(),
                    None => set.clone(),
                };
                let ok = !next.is_empty();
                self.req[v] = Some(next);
                ok
            }
        }
    }

    /// What the variable may hold now (`None`: any).
    pub fn now(&self, v: usize) -> Option<Set> {
        match &self.cur[v] {
            Some(Some(x)) => Some(Set::from([x.clone()])),
            Some(None) => None,
            None => self.req[v].clone(),
        }
    }
}

/// The paths leaving a block.
#[derive(Default)]
struct Flowed {
    falls: Vec<Path>,
    returns: Vec<Path>,
    breaks: Vec<Path>,
}

pub struct Walker<'a> {
    pub index: &'a Index<'a>,
    pub flows: &'a dyn FlowSource,
    pub vars: Vec<Var>,
    pub events: Vec<Event>,
    /// Reads of a tracked variable in conditions (evidence that the state matters).
    pub reads: usize,
    owner: Option<&'a Symbol>,
    trigger: String,
    method: String,
    bindings: HashMap<String, String>,
    depth: usize,
    language: Language,
}

/// Normal ends of a walked function (for rules over several variables).
pub struct Exits {
    pub paths: Vec<Path>,
}

impl<'a> Walker<'a> {
    pub fn new(index: &'a Index<'a>, flows: &'a dyn FlowSource, vars: Vec<Var>) -> Self {
        Self {
            index,
            flows,
            vars,
            events: Vec::new(),
            reads: 0,
            owner: None,
            trigger: String::new(),
            method: String::new(),
            bindings: HashMap::new(),
            depth: 0,
            language: Language::Csharp,
        }
    }

    /// Some values are only known once every function was read (string literals): forget the
    /// transitions read so far and read again with the whole domain. False when nothing changes.
    pub fn second_pass(&mut self) -> bool {
        if !self.vars.iter().any(|v| v.open) {
            return false;
        }
        self.events.clear();
        self.reads = 0;
        true
    }

    fn domain(&self, v: usize) -> Set {
        self.vars[v].domain.iter().cloned().collect()
    }

    fn accept(&mut self, v: usize, value: String) -> Option<String> {
        if self.vars[v].domain.contains(&value) {
            return Some(value);
        }
        if self.vars[v].open {
            self.vars[v].domain.push(value.clone());
            return Some(value);
        }
        None
    }

    /// Walks function `f` (a method of `owner`) from fresh paths, as `trigger`.
    pub fn function(&mut self, f: &Symbol, owner: Option<&'a Symbol>, trigger: &str) -> Exits {
        let Some(flow) = self.flows.flow(&f.id) else { return Exits { paths: Vec::new() } };
        self.owner = owner;
        self.trigger = trigger.to_owned();
        self.method = f.id.clone();
        self.language = f.language;
        self.bindings.clear();
        let body = tree(&flow);
        let start = vec![Path::new(self.vars.len())];
        let out = self.block(&flow, &body, start);
        let mut paths = out.falls;
        paths.extend(out.returns);
        Exits { paths }
    }

    /// Walks one block (a loop body) from `start`, as `trigger`.
    pub fn block_of(&mut self, f: &Symbol, flow: &FlowGraph, block: &Block, start: Vec<Path>, trigger: &str) -> Vec<Path> {
        self.trigger = trigger.to_owned();
        self.method = f.id.clone();
        self.language = f.language;
        let out = self.block(flow, block, start);
        let mut paths = out.falls;
        paths.extend(out.breaks);
        paths
    }

    /// (values when true, values when false) of a condition, for variable `v`.
    fn sets(&mut self, v: usize, e: &Expr) -> Option<(Set, Set)> {
        let d = self.domain(v);
        let minus = |d: &Set, s: &Set| d.difference(s).cloned().collect::<Set>();
        match e.op.as_deref() {
            Some("and") if e.children.len() == 2 => {
                let a = self.sets(v, &e.children[0]);
                let b = self.sets(v, &e.children[1]);
                return match (a, b) {
                    (None, None) => None,
                    (Some(a), None) | (None, Some(a)) => Some((a.0, d)),
                    (Some(a), Some(b)) => Some((a.0.intersection(&b.0).cloned().collect(), a.1.union(&b.1).cloned().collect())),
                };
            }
            Some("or") if e.children.len() == 2 => {
                let a = self.sets(v, &e.children[0]);
                let b = self.sets(v, &e.children[1]);
                return match (a, b) {
                    (None, None) => None,
                    (Some(a), None) | (None, Some(a)) => Some((d, a.1)),
                    (Some(a), Some(b)) => Some((a.0.union(&b.0).cloned().collect(), a.1.intersection(&b.1).cloned().collect())),
                };
            }
            Some("not") if e.children.len() == 1 => return self.sets(v, &e.children[0]).map(|(t, f)| (f, t)),
            Some(op @ ("equal" | "loose_equal" | "not_equal" | "loose_not_equal")) if e.children.len() == 2 => {
                let (a, b) = (&e.children[0], &e.children[1]);
                let other = if self.vars[v].matcher.reads(a) {
                    b
                } else if self.vars[v].matcher.reads(b) {
                    a
                } else {
                    return None;
                };
                let value = literal_value(other, &self.bindings).and_then(|x| self.accept(v, x))?;
                let d = self.domain(v);
                let yes = Set::from([value]);
                let no = minus(&d, &yes);
                self.reads += 1;
                return Some(if op.starts_with("not") || op.contains("not_") { (no, yes) } else { (yes, no) });
            }
            _ => {}
        }
        match e.concept {
            // A boolean flag tested directly: `if (_disposed)`.
            Concept::Var | Concept::GetMember if self.vars[v].matcher.reads(e) => {
                self.reads += 1;
                Some((Set::from(["true".to_owned()]), Set::from(["false".to_owned()])))
            }
            Concept::TypeCheck if e.children.first().is_some_and(|c| self.vars[v].matcher.reads(c)) => {
                let ty = e.label.trim_start_matches("is ").split_whitespace().next().unwrap_or("").to_owned();
                let ty = self.accept(v, ty)?;
                let yes = Set::from([ty]);
                self.reads += 1;
                Some((yes.clone(), minus(&self.domain(v), &yes)))
            }
            Concept::Await | Concept::SafeAccess if e.children.len() == 1 => self.sets(v, &e.children[0]),
            _ => None,
        }
    }

    fn dedupe(paths: Vec<Path>) -> Vec<Path> {
        let mut set: BTreeSet<Path> = paths.into_iter().collect();
        while set.len() > MAX_PATHS {
            set.pop_last();
        }
        set.into_iter().collect()
    }

    fn cond_text(node: &FlowNode) -> String {
        node.label.trim_start_matches("if ").trim().to_owned()
    }

    fn block(&mut self, flow: &FlowGraph, block: &Block, mut paths: Vec<Path>) -> Flowed {
        let mut out = Flowed::default();
        for step in &block.steps {
            if paths.is_empty() {
                break;
            }
            let node = step.node;
            match node.concept {
                Concept::Branch => {
                    paths = self.simple(flow, node, paths);
                    let Some(cond) = &node.expr else { continue };
                    let (mut yes, mut no) = (Vec::new(), Vec::new());
                    for p in paths {
                        let (mut py, mut pn) = (p.clone(), p);
                        let (mut fy, mut fnn, mut any) = (true, true, false);
                        for v in 0..self.vars.len() {
                            if let Some((t, f)) = self.sets(v, cond) {
                                any = true;
                                fy &= py.narrow(v, &t);
                                fnn &= pn.narrow(v, &f);
                            }
                        }
                        if !any {
                            let text = Self::cond_text(node);
                            pn.guards.push(negate(&text));
                            py.guards.push(text);
                        }
                        if fy {
                            yes.push(py);
                        }
                        if fnn {
                            no.push(pn);
                        }
                    }
                    let mut next = Vec::new();
                    for (pin, start) in [("true", yes), ("false", no)] {
                        let r = match step.arm(pin) {
                            Some(a) => self.block(flow, &a.block, start),
                            None => Flowed { falls: start, ..Flowed::default() },
                        };
                        next.extend(r.falls);
                        out.returns.extend(r.returns);
                        out.breaks.extend(r.breaks);
                    }
                    paths = Self::dedupe(next);
                }
                Concept::Match | Concept::BuildSwitch => {
                    paths = self.simple(flow, node, paths);
                    let var = node.expr.as_ref().and_then(|e| (0..self.vars.len()).find(|v| self.vars[*v].matcher.reads(e)));
                    let fallthrough = self.language != Language::Rust;
                    let mut next = Vec::new();
                    let mut listed = Set::new();
                    let mut pending: Option<Set> = None;
                    let mut has_default = false;
                    for arm in &step.arms {
                        let values = case_values(&arm.label);
                        let set: Option<Set> = match (var, &values) {
                            (Some(v), Some(vals)) => {
                                let mut s = Set::new();
                                for x in vals {
                                    if let Some(x) = self.accept(v, x.clone()) {
                                        s.insert(x);
                                    }
                                }
                                listed.extend(s.iter().cloned());
                                Some(s)
                            }
                            (Some(_), None) => {
                                has_default = true;
                                None
                            }
                            _ => None,
                        };
                        // C# and TypeScript: an empty case falls into the next one.
                        if fallthrough && arm.block.is_empty() && values.is_some() {
                            let acc = pending.get_or_insert_with(Set::new);
                            acc.extend(set.unwrap_or_default());
                            continue;
                        }
                        let mut set = set;
                        if let (Some(p), Some(s)) = (pending.take(), set.as_mut()) {
                            s.extend(p);
                        }
                        let mut start = Vec::new();
                        for p in &paths {
                            let mut p = p.clone();
                            match (var, &set) {
                                (Some(v), Some(s)) => {
                                    self.reads += 1;
                                    if !p.narrow(v, s) {
                                        continue;
                                    }
                                }
                                (Some(v), None) => {
                                    let rest: Set = self.domain(v).difference(&listed).cloned().collect();
                                    if !p.narrow(v, &rest) {
                                        continue;
                                    }
                                }
                                (None, _) => p.guards.push(format!("{} is {}", node.label.trim_start_matches("match "), arm.label.trim_end_matches(':'))),
                            }
                            start.push(p);
                        }
                        let r = self.block(flow, &arm.block, start);
                        next.extend(r.falls);
                        out.returns.extend(r.returns);
                        // `break` ends a switch section.
                        if var.is_some() {
                            next.extend(r.breaks);
                        } else {
                            out.breaks.extend(r.breaks);
                        }
                    }
                    // No default case: the other values skip the switch.
                    match var {
                        Some(v) if !has_default => {
                            let rest: Set = self.domain(v).difference(&listed).cloned().collect();
                            for p in &paths {
                                let mut p = p.clone();
                                if !rest.is_empty() && p.narrow(v, &rest) {
                                    next.push(p);
                                }
                            }
                        }
                        None if !step.arms.iter().any(|a| case_values(&a.label).is_none()) => next.extend(paths.iter().cloned()),
                        _ => {}
                    }
                    paths = Self::dedupe(next);
                }
                Concept::LoopWhile | Concept::LoopRange | Concept::LoopForEach | Concept::LoopInfinite => {
                    paths = self.simple(flow, node, paths);
                    if let Some(body) = step.arm("body") {
                        // The body runs zero times or once (enough to see what it changes).
                        let r = self.block(flow, &body.block, paths.clone());
                        out.returns.extend(r.returns);
                        if node.concept == Concept::LoopInfinite {
                            paths = r.breaks;
                        } else {
                            paths.extend(r.falls);
                            paths.extend(r.breaks);
                        }
                        paths = Self::dedupe(paths);
                    }
                }
                Concept::Try => {
                    let entry = paths.clone();
                    let mut next = Vec::new();
                    if let Some(a) = step.arm("try") {
                        let r = self.block(flow, &a.block, paths);
                        next.extend(r.falls);
                        out.returns.extend(r.returns);
                        out.breaks.extend(r.breaks);
                    }
                    for arm in step.arms.iter().filter(|a| a.pin.starts_with("catch:")) {
                        // An error can come before anything in the try ran.
                        let r = self.block(flow, &arm.block, entry.clone());
                        next.extend(r.falls);
                        out.returns.extend(r.returns);
                        out.breaks.extend(r.breaks);
                    }
                    paths = Self::dedupe(next);
                    if let Some(f) = step.arm("finally") {
                        let r = self.block(flow, &f.block, paths);
                        paths = r.falls;
                        out.returns.extend(r.returns);
                    }
                }
                Concept::Scope | Concept::CriticalSection => {
                    paths = self.simple(flow, node, paths);
                    if let Some(b) = step.arm("body") {
                        let r = self.block(flow, &b.block, paths);
                        paths = r.falls;
                        out.returns.extend(r.returns);
                        out.breaks.extend(r.breaks);
                    }
                }
                Concept::Return => {
                    paths = self.simple(flow, node, paths);
                    out.returns.append(&mut paths);
                }
                Concept::Throw | Concept::PropagateError => paths.clear(),
                Concept::Break | Concept::Continue => out.breaks.append(&mut paths),
                _ => {
                    paths = self.simple(flow, node, paths);
                    for arm in step.arms.iter().filter(|a| a.pin != "error") {
                        let r = self.block(flow, &arm.block, paths.clone());
                        out.returns.extend(r.returns);
                    }
                    if !step.falls {
                        paths.clear();
                    }
                }
            }
        }
        out.falls = paths;
        out
    }

    /// Calls into helpers of the same type, then assignments of the statement.
    fn simple(&mut self, flow: &FlowGraph, node: &FlowNode, mut paths: Vec<Path>) -> Vec<Path> {
        for site in call_sites(node) {
            let own = site.receiver.is_none_or(|r| r.concept == Concept::Var && matches!(r.label.as_str(), "this" | "self"));
            // React setters.
            for v in 0..self.vars.len() {
                if let Matcher::Setter { setter, .. } = &self.vars[v].matcher
                    && *setter == site.name
                {
                    let value = site.expr.children.iter().find(|c| !c.flavors.iter().any(|f| f == "target")).and_then(|a| literal_value(a, &self.bindings));
                    let value = value.and_then(|x| self.accept(v, x));
                    paths = self.assign(paths, v, value, &site.expr.span);
                }
            }
            if !own || self.depth >= 2 {
                continue;
            }
            let Some(owner) = self.owner else { continue };
            let helper = self.index.children.get(owner.id.as_str()).into_iter().flatten().find(|m| m.name == site.name && m.has_flow && m.id != self.method && matches!(m.kind, SymbolKind::Method | SymbolKind::Function));
            let Some(helper) = helper else { continue };
            let Some(hflow) = self.flows.flow(&helper.id) else { continue };
            // Arguments bound to the helper's parameters (`EnsureStatus(OrderStatus.Submitted)`).
            let params = helper.signature.as_deref().map(|s| parameter_types(s, &helper.name, helper.language == Language::Csharp)).unwrap_or_default();
            let args: Vec<&Expr> = site.expr.children.iter().filter(|c| !c.flavors.iter().any(|f| f == "target")).collect();
            let saved = std::mem::take(&mut self.bindings);
            for ((name, _), arg) in params.iter().zip(args) {
                if let Some(v) = literal_value(arg, &saved) {
                    self.bindings.insert(name.clone(), v);
                }
            }
            self.depth += 1;
            let body = tree(&hflow);
            let r = self.block(&hflow, &body, paths);
            self.depth -= 1;
            self.bindings = saved;
            let mut next = r.falls;
            next.extend(r.returns);
            paths = Self::dedupe(next);
        }
        // `x = value`, `self.state = State::B`, `var mode = Mode.Start`.
        if let Some(e) = &node.expr {
            let target_value = if e.concept == Concept::Assign && e.label == "=" && e.children.len() == 2 {
                Some((&e.children[0], Some(&e.children[1])))
            } else {
                None
            };
            for v in 0..self.vars.len() {
                let assigned = match (&self.vars[v].matcher, target_value) {
                    (m, Some((t, value))) if m.reads(t) => Some(value),
                    (Matcher::Local(n), _) if matches!(node.concept, Concept::Var) && node.writes.iter().any(|w| w == n) => Some(Some(e)),
                    _ => None,
                };
                if let Some(value) = assigned {
                    let value = value.and_then(|x| literal_value(x, &self.bindings)).and_then(|x| self.accept(v, x));
                    paths = self.assign(paths, v, value, &node.span);
                }
            }
        }
        let _ = flow;
        paths
    }

    fn assign(&mut self, paths: Vec<Path>, v: usize, value: Option<String>, site: &Span) -> Vec<Path> {
        paths
            .into_iter()
            .map(|mut p| {
                if let Some(to) = &value {
                    let from = p.now(v);
                    let guard = (!p.guards.is_empty()).then(|| p.guards.join(" and "));
                    self.events.push(Event { var: v, from, to: to.clone(), trigger: self.trigger.clone(), guard, method: self.method.clone(), site: site.clone() });
                }
                p.cur[v] = Some(value.clone());
                p
            })
            .collect()
    }
}
