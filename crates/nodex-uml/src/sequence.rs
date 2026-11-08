//! Static sequence diagrams: who a function talks to, in order, as a Mermaid `sequenceDiagram`.
//!
//! The root function's flow is walked statement by statement; every call becomes a message to
//! the participant that owns the callee (its type, its module, or its component function), and
//! calls into project code are followed into the callee's own flow up to a depth. Control
//! structures become fragments (`alt`/`opt` for `if`/`switch`, `loop`, `critical`/`option` for
//! `try`/`catch`, `par` for `Task.WhenAll`/`Promise.all`/`join!`). The knowledge graph adds what
//! the code alone does not say: an HTTP request goes to the route handler in the other component
//! (front end to back end), interface calls go to the implementation dependency injection
//! provides, database access goes to a database participant, and an uncaught exception goes to
//! the framework's global error handler. Library calls that are not infrastructure (logging,
//! formatting, collections) are left out unless asked for.

use std::collections::{BTreeMap, HashMap, HashSet};

use nodex_core::units::BuildUnit;
use nodex_ir::knowledge::{Confidence, EntryKind};
use nodex_ir::{Concept, FlowGraph, FlowNode, Span, StructEdgeKind, Symbol, SymbolKind};

use crate::calls::{Callee, Resolver};
use crate::flowtree::{Block, CallSite, Step, call_sites, thrown_type, tree, within};
use crate::index::Index;
use crate::mermaid::shorten;
use crate::roots::{function_label, module_name, roots};
use crate::scope::Scoped;
use crate::{UmlCandidate, UmlDiagram, UmlElement, UmlInput, UmlKind, UmlOptions, UmlRelation, UmlScope, empty};
use nodex_trace::CallSpan;

const MAX_MESSAGES: usize = 400;
const DEFAULT_DEPTH: u32 = 3;
/// Marks an inferred message until the end decides whether marks are worth showing.
const INFERRED: &str = "\u{1}";

struct Part {
    id: String,
    label: String,
    symbol: Option<String>,
    kind: &'static str,
    unit: Option<String>,
    /// Declared where it is first created (`create participant`), not in the header.
    created: bool,
    span: Option<Span>,
}

struct Body<'f> {
    flow: &'f FlowGraph,
    sym: &'f Symbol,
}

struct Seq<'a> {
    index: &'a Index<'a>,
    resolver: Resolver<'a>,
    input: &'a UmlInput<'a>,
    opts: &'a UmlOptions,
    parts: Vec<Part>,
    by_key: HashMap<String, usize>,
    lines: Vec<String>,
    indent: usize,
    messages: usize,
    truncated: bool,
    relations: Vec<UmlRelation>,
    stack: Vec<String>,
    notes: HashSet<(String, String)>,
    exact: usize,
    inferred: usize,
    hidden: usize,
}

pub fn build(index: &Index, scoped: &Scoped, scope: &UmlScope, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut d = empty(UmlKind::Sequence);
    let (candidates, chosen) = roots(index, scoped, scope, UmlKind::Sequence, opts);
    d.candidates = candidates;
    let Some(root) = chosen else {
        d.warnings.push("No function with a body in this scope: choose a method, a function or code inside one.".into());
        return d;
    };
    let Some(sym) = index.get(&root) else { return d };
    if input.flows.flow(&root).is_none() {
        d.warnings.push(format!("The body of {} is not available.", sym.name));
        return d;
    }
    let mut s = Seq::new(index, input, opts);
    let info = index.info.get(root.as_str());
    let actor = s.actor(sym);
    let me = s.function_part(sym);
    let trigger = info.and_then(|i| i.entry.as_ref()).map(|e| e.label.clone()).unwrap_or_else(|| format!("{}()", sym.name));
    s.message(&actor, &me, "->>+", &trigger, None, "entry", None, None);
    let depth = opts.depth.unwrap_or(DEFAULT_DEPTH).min(8);
    let ret = s.flow(&root, &me, &actor, depth);
    let is_route = info.and_then(|i| i.entry.as_ref()).is_some_and(|e| e.kind == EntryKind::Route);
    let ret = if is_route { Some(ret.map(|r| format!("HTTP response: {r}")).unwrap_or_else(|| "HTTP response".into())) } else { ret };
    s.close(&me, &actor, ret);

    d.title = format!("Sequence of {}", function_label(index, sym));
    s.finish(&mut d);
    d
}

/// Text for a message, a fragment or a participant (no quotes; `;` and `#` would end it).
fn seq_text(text: &str, max: usize) -> String {
    let short = shorten(text, max);
    let mut out = String::with_capacity(short.len());
    for ch in short.chars() {
        match ch {
            '#' => out.push_str("#35;"),
            ';' => out.push_str("#59;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

fn is_function(s: &Symbol) -> bool {
    matches!(s.kind, SymbolKind::Method | SymbolKind::Function | SymbolKind::Constructor)
}

impl<'a> Seq<'a> {
    fn new(index: &'a Index<'a>, input: &'a UmlInput<'a>, opts: &'a UmlOptions) -> Self {
        Seq {
            index,
            resolver: Resolver::new(index),
            input,
            opts,
            parts: Vec::new(),
            by_key: HashMap::new(),
            lines: Vec::new(),
            indent: 1,
            messages: 0,
            truncated: false,
            relations: Vec::new(),
            stack: Vec::new(),
            notes: HashSet::new(),
            exact: 0,
            inferred: 0,
            hidden: 0,
        }
    }

    /// Writes the text and the element list into the diagram.
    fn finish(self, d: &mut UmlDiagram) {
        let mut out = String::from("sequenceDiagram\n");
        self.header(&mut out);
        let mark = self.exact > 0 && self.inferred > 0;
        for l in &self.lines {
            out.push_str(&l.replace(INFERRED, if mark { " ≈" } else { "" }));
            out.push('\n');
        }
        d.mermaid = out;
        d.elements = self
            .parts
            .iter()
            .map(|p| UmlElement {
                mermaid_id: p.id.clone(),
                symbol: p.symbol.clone(),
                label: p.label.clone(),
                kind: p.kind.to_owned(),
                span: p.span.clone(),
                context: false,
                detail: p.unit.clone(),
            })
            .collect();
        d.relations = self.relations;
        d.truncated = self.truncated;
        d.stats.hidden = self.hidden as u32;
        if self.truncated {
            d.warnings.push(format!("Only the first {MAX_MESSAGES} messages are drawn; lower the depth or start from a callee."));
        }
        if self.inferred > 0 && self.exact == 0 {
            d.warnings.push("Calls were resolved from declared types and names (no compiler analysis yet): treat them as likely, not certain.".into());
        } else if mark {
            d.warnings.push("Messages marked ≈ were resolved from declared types and names, not by a compiler.".into());
        }
    }

    fn line(&mut self, text: String) {
        self.lines.push(format!("{}{text}", "  ".repeat(self.indent)));
    }

    fn unit_of(&self, file: &str) -> Option<String> {
        let units: &[BuildUnit] = self.input.units;
        let unit = units.iter().filter(|u| u.root.is_empty() || file.starts_with(&format!("{}/", u.root))).max_by_key(|u| u.root.len());
        match unit {
            Some(u) => Some(u.name.clone()),
            // Outside every build unit: its top-level folder, as in component diagrams.
            None => file.split_once('/').map(|(top, _)| top.to_owned()),
        }
    }

    fn part(&mut self, key: &str, label: &str, symbol: Option<&Symbol>, kind: &'static str) -> String {
        if let Some(&i) = self.by_key.get(key) {
            return self.parts[i].id.clone();
        }
        let id = format!("n{}", self.parts.len());
        let unit = symbol.and_then(|s| s.origins.first()).and_then(|o| self.unit_of(&o.file));
        self.parts.push(Part {
            id: id.clone(),
            label: label.to_owned(),
            symbol: symbol.map(|s| s.id.clone()),
            kind,
            unit,
            created: false,
            span: symbol.and_then(|s| s.origins.first().cloned()),
        });
        self.by_key.insert(key.to_owned(), self.parts.len() - 1);
        id
    }

    /// The participant that runs a function: its type, the component function it is nested
    /// in (a React component's handlers), or its module.
    fn function_part(&mut self, f: &Symbol) -> String {
        if let Some(t) = self.index.owner_type(&f.id).filter(|t| t.id != f.id) {
            let kind = if self.index.routed.contains_key(&t.id) { "controller" } else { "participant" };
            return self.part(&t.id, &t.name, Some(t), kind);
        }
        // The outermost function under the module.
        let mut outer = f;
        while let Some(p) = outer.parent.as_deref().and_then(|p| self.index.get(p)) {
            if is_function(p) {
                outer = p;
            } else {
                break;
            }
        }
        if outer.id != f.id {
            return self.part(&outer.id, &outer.name, Some(outer), "participant");
        }
        match self.index.namespace_of(&f.id) {
            Some(m) => {
                let name = if m.kind == SymbolKind::Module { module_name(m) } else { m.name.clone() };
                self.part(&m.id, &name, Some(m), "participant")
            }
            None => self.part(&f.id, &f.name, Some(f), "participant"),
        }
    }

    /// Who starts the root function, from its entry point.
    fn actor(&mut self, root: &Symbol) -> String {
        let info = self.index.info.get(root.id.as_str());
        let entry = info.and_then(|i| i.entry.as_ref());
        let auth = info.and_then(|i| i.auth.as_ref());
        let needs_auth = info.is_some_and(|i| i.capabilities.iter().any(|c| c == "AUTH_REQUIRED"));
        let (label, human) = match entry.map(|e| e.kind) {
            Some(EntryKind::Route) => match auth {
                Some(a) if !a.roles.is_empty() => (format!("{} user", a.roles.join(" / ")), true),
                Some(a) if a.anonymous => ("Visitor".to_owned(), true),
                _ if needs_auth => ("Authenticated client".to_owned(), true),
                _ => ("Client".to_owned(), true),
            },
            Some(EntryKind::Ui) => ("User".to_owned(), true),
            Some(EntryKind::Main | EntryKind::Command) => ("Operator".to_owned(), true),
            Some(EntryKind::Job) => ("Scheduler".to_owned(), false),
            Some(EntryKind::Message) => ("Message broker".to_owned(), false),
            Some(EntryKind::EventHandler) => ("Event source".to_owned(), false),
            Some(EntryKind::Test) => ("Test runner".to_owned(), false),
            Some(EntryKind::ServerStart) => ("Host".to_owned(), false),
            Some(EntryKind::Framework) => {
                let fw = if root.flavors.iter().any(|f| f.starts_with("react:")) {
                    "React"
                } else if root.flavors.iter().any(|f| f.starts_with("unity")) || root.flavors.iter().any(|f| f.starts_with("attr:")) && root.language == nodex_ir::Language::Csharp {
                    "Unity Engine"
                } else {
                    "Framework"
                };
                (fw.to_owned(), false)
            }
            Some(EntryKind::Export) | None => {
                // A handler declared inside a UI component runs when the user acts.
                let in_component = root.parent.as_deref().and_then(|p| self.index.get(p)).is_some_and(|p| p.flavors.iter().any(|f| f.starts_with("react:") || f.starts_with("vue:")));
                if in_component { ("User".to_owned(), true) } else { ("Caller".to_owned(), false) }
            }
        };
        self.part("actor:", &label, None, if human { "actor" } else { "participant" })
    }

    /// The database a participant talks to: the data models of its build unit tell which
    /// (each back end of a full-stack project has its own).
    fn database(&mut self, from: &str) -> String {
        let unit = self.parts.iter().find(|p| p.id == from).and_then(|p| p.unit.clone());
        let unit_of_model = |m: &nodex_ir::knowledge::ModelInfo| self.index.file_of(&m.symbol).and_then(|f| self.unit_of(f));
        let mine: std::collections::BTreeSet<&str> =
            self.input.knowledge.models.iter().filter(|m| unit.is_some() && unit_of_model(m) == unit).map(|m| m.model.framework.as_str()).collect();
        let fw: std::collections::BTreeSet<&str> =
            if mine.is_empty() { self.input.knowledge.models.iter().map(|m| m.model.framework.as_str()).collect() } else { mine.clone() };
        let label = match fw.len() {
            0 => "Database".to_owned(),
            1 => crate::component::db_name(fw.first().copied().unwrap_or_default(), None),
            _ => format!("Database ({})", fw.into_iter().map(|f| crate::component::db_name(f, None).trim_start_matches("Database (").trim_end_matches(')').to_owned()).collect::<Vec<_>>().join(", ")),
        };
        let key = if mine.is_empty() { "infra:database".to_owned() } else { format!("infra:database:{}", unit.unwrap_or_default()) };
        self.part(&key, &label, None, "database")
    }

    fn infra(&mut self, kind: &'static str) -> String {
        let label = match kind {
            "database" => {
                let fw: std::collections::BTreeSet<&str> = self.input.knowledge.models.iter().map(|m| m.model.framework.as_str()).collect();
                match fw.len() {
                    0 => "Database".to_owned(),
                    1 => crate::component::db_name(fw.first().copied().unwrap_or_default(), None),
                    _ => format!("Database ({})", fw.into_iter().map(|f| crate::component::db_name(f, None).trim_start_matches("Database (").trim_end_matches(')').to_owned()).collect::<Vec<_>>().join(", ")),
                }
            }
            "cache" => "Cache".to_owned(),
            "files" => "File system".to_owned(),
            "storage" => "Browser storage".to_owned(),
            "events" => "Event bus".to_owned(),
            "errors" => "Error handler".to_owned(),
            _ => kind.to_owned(),
        };
        self.part(&format!("infra:{kind}"), &label, None, kind)
    }

    #[allow(clippy::too_many_arguments)]
    fn message(&mut self, from: &str, to: &str, arrow: &str, text: &str, confidence: Option<Confidence>, kind: &str, evidence: Option<String>, site: Option<&Span>) {
        if self.messages >= MAX_MESSAGES {
            self.truncated = true;
            return;
        }
        self.messages += 1;
        let mark = match confidence {
            Some(Confidence::Exact) => {
                self.exact += 1;
                ""
            }
            Some(_) => {
                self.inferred += 1;
                INFERRED
            }
            None => "",
        };
        let body = seq_text(text, 70);
        let body = if body.is_empty() { " ".to_owned() } else { body };
        self.line(format!("{from}{arrow}{to}: {body}{mark}"));
        if confidence.is_some() || kind != "entry" {
            self.relations.push(UmlRelation {
                from: from.to_owned(),
                to: to.to_owned(),
                kind: kind.to_owned(),
                label: Some(text.to_owned()),
                confidence: confidence.filter(|c| *c != Confidence::Exact),
                evidence,
                site: site.cloned(),
                count: 1,
            });
        }
    }

    /// Ends an activation: a reply when there is something to say, else a plain deactivation.
    fn close(&mut self, me: &str, caller: &str, ret: Option<String>) {
        match ret {
            Some(r) if me != caller => {
                self.messages += 1;
                self.line(format!("{me}-->>-{caller}: {}", seq_text(&r, 50)));
            }
            _ => self.line(format!("deactivate {me}")),
        }
    }

    /// Walks a function's flow as `me`, called by `caller`. Returns the text of its final
    /// return, or its return type.
    fn flow(&mut self, id: &str, me: &str, caller: &str, depth: u32) -> Option<String> {
        let flow = self.input.flows.flow(id)?;
        let sym = self.index.get(id)?;
        self.stack.push(id.to_owned());
        let body = Body { flow: &flow, sym };
        let tree = tree(&flow);
        let last = self.block(&body, &tree, me, caller, depth, true);
        self.stack.pop();
        last.or_else(|| sym.type_text.clone().filter(|t| !matches!(t.as_str(), "void" | "Task" | "ValueTask" | "Promise<void>" | "()" | "")))
    }

    /// Emits a block; `top` when it is the function body itself (its last return is the reply).
    fn block(&mut self, b: &Body, block: &Block, me: &str, caller: &str, depth: u32, top: bool) -> Option<String> {
        let mut last = None;
        for (i, step) in block.steps.iter().enumerate() {
            let is_last = top && i + 1 == block.steps.len();
            last = self.step(b, step, me, caller, depth, is_last);
        }
        last
    }

    /// Runs `f` into a separate buffer; returns the lines and whether they hold a message.
    fn capture(&mut self, f: impl FnOnce(&mut Self)) -> (Vec<String>, bool) {
        let saved = std::mem::take(&mut self.lines);
        let before = self.messages;
        self.indent += 1;
        f(self);
        self.indent -= 1;
        let lines = std::mem::replace(&mut self.lines, saved);
        (lines, self.messages > before)
    }

    fn fragment(&mut self, sections: Vec<(String, Vec<String>, bool)>, keyword: &str, other: &str) {
        if !sections.iter().any(|(_, _, has)| *has) {
            return;
        }
        for (i, (title, lines, _)) in sections.into_iter().enumerate() {
            let kw = if i == 0 { keyword } else { other };
            let title = seq_text(&title, 60);
            self.line(if title.is_empty() { kw.to_owned() } else { format!("{kw} {title}") });
            self.lines.extend(lines);
        }
        self.line("end".into());
    }

    fn step(&mut self, b: &Body, step: &Step, me: &str, caller: &str, depth: u32, last: bool) -> Option<String> {
        let node = step.node;
        match node.concept {
            Concept::Branch => {
                self.calls(b, node, me, depth);
                let cond = node.label.trim_start_matches("if ").to_owned();
                let yes = step.arm("true");
                let no = step.arm("false");
                let (yl, yh) = match yes {
                    Some(a) => self.capture(|s| {
                        s.block(b, &a.block, me, caller, depth, false);
                    }),
                    None => (Vec::new(), false),
                };
                let (nl, nh) = match no {
                    Some(a) => self.capture(|s| {
                        s.block(b, &a.block, me, caller, depth, false);
                    }),
                    None => (Vec::new(), false),
                };
                match (yh, nh) {
                    (true, true) => self.fragment(vec![(cond, yl, true), ("else".into(), nl, true)], "alt", "else"),
                    (true, false) => self.fragment(vec![(cond, yl, true)], "opt", ""),
                    (false, true) => self.fragment(vec![(format!("not ({cond})"), nl, true)], "opt", ""),
                    (false, false) => {}
                }
                None
            }
            Concept::Match | Concept::BuildSwitch => {
                self.calls(b, node, me, depth);
                let mut sections = Vec::new();
                for arm in &step.arms {
                    let (l, has) = self.capture(|s| {
                        s.block(b, &arm.block, me, caller, depth, false);
                    });
                    let title = format!("{}: {}", node.label.trim_start_matches("match "), arm.label.trim_end_matches(':'));
                    sections.push((title, l, has));
                }
                // Cases without messages are not drawn; the first drawn one opens the fragment.
                let kept: Vec<_> = sections.into_iter().filter(|s| s.2).collect();
                self.fragment(kept, "alt", "else");
                None
            }
            Concept::LoopWhile | Concept::LoopRange | Concept::LoopForEach | Concept::LoopInfinite => {
                self.calls(b, node, me, depth);
                if let Some(body) = step.arm("body") {
                    let (l, has) = self.capture(|s| {
                        s.block(b, &body.block, me, caller, depth, false);
                    });
                    self.fragment(vec![(node.label.clone(), l, has)], "loop", "");
                }
                None
            }
            Concept::Try => {
                let mut sections = Vec::new();
                if let Some(body) = step.arm("try") {
                    let (l, has) = self.capture(|s| {
                        s.block(b, &body.block, me, caller, depth, false);
                    });
                    sections.push(("try".to_owned(), l, has));
                }
                for arm in step.arms.iter().filter(|a| a.pin.starts_with("catch:")) {
                    let (l, has) = self.capture(|s| {
                        s.block(b, &arm.block, me, caller, depth, false);
                    });
                    let what = match arm.ty.as_deref() {
                        Some("*") | None => "any error".to_owned(),
                        Some(t) => t.to_owned(),
                    };
                    sections.push((format!("catch {what}"), l, has));
                }
                // Catch clauses that send nothing are left out (the first section is the try).
                let mut first = true;
                sections.retain(|s| std::mem::replace(&mut first, false) || s.2);
                if sections.iter().any(|s| s.2) {
                    // Mermaid needs a body under `critical`; an empty one gets a note.
                    if !sections[0].2 {
                        sections[0].1.push(format!("{}Note over {me}: nothing to show", "  ".repeat(self.indent + 1)));
                    }
                    self.fragment(sections, "critical", "option");
                }
                if let Some(fin) = step.arm("finally") {
                    self.block(b, &fin.block, me, caller, depth, false);
                }
                None
            }
            Concept::CriticalSection => {
                if let Some(body) = step.arm("body") {
                    let (l, has) = self.capture(|s| {
                        s.block(b, &body.block, me, caller, depth, false);
                    });
                    self.fragment(vec![(node.label.clone(), l, has)], "critical", "");
                }
                None
            }
            Concept::Scope => {
                self.calls(b, node, me, depth);
                if let Some(body) = step.arm("body") {
                    self.block(b, &body.block, me, caller, depth, false);
                }
                None
            }
            Concept::Return => {
                self.calls(b, node, me, depth);
                let value = node.code.trim().trim_start_matches("return").trim().trim_end_matches(';').trim().to_owned();
                // A component returns its markup: name the element it renders.
                let value = if node.flavors.iter().any(|f| f == "jsx") {
                    let tag = value.trim_start_matches('(').trim().split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("").trim_start_matches('<').to_owned();
                    format!("renders <{tag}>")
                } else {
                    value
                };
                let value = (!value.is_empty()).then_some(value);
                if last {
                    return value;
                }
                // An early return: a reply that does not end the activation.
                if me != caller {
                    let text = value.unwrap_or_else(|| "return".into());
                    self.message(me, caller, "-->>", &text, None, "return", None, Some(&node.span));
                }
                None
            }
            Concept::Throw | Concept::PropagateError => {
                self.calls(b, node, me, depth);
                let ty = thrown_type(node).unwrap_or_else(|| "error".into());
                // A framework's global handler takes what nothing on the way catches.
                let global = self.index.graph.edges.iter().find(|e| {
                    e.kind == StructEdgeKind::CaughtBy && e.from == b.sym.id && e.evidence.as_deref().is_some_and(|ev| ev.contains("global handler"))
                });
                let (to, evidence) = match global {
                    Some(e) => (self.infra("errors"), e.evidence.clone()),
                    None => (caller.to_owned(), None),
                };
                self.message(me, &to, "--x", &format!("throw {ty}"), None, "throw", evidence, Some(&node.span));
                None
            }
            Concept::Spawn => {
                self.calls_with(b, node, me, depth, true);
                None
            }
            Concept::Emit => {
                self.calls(b, node, me, depth);
                let bus = self.infra("events");
                self.message(me, &bus, "-)", &node.label, None, "event", None, Some(&node.span));
                None
            }
            _ => {
                self.calls(b, node, me, depth);
                for arm in &step.arms {
                    if arm.pin == "error" {
                        continue;
                    }
                    let (l, has) = self.capture(|s| {
                        s.block(b, &arm.block, me, caller, depth, false);
                    });
                    let title = if arm.pin == "else" { format!("otherwise: {}", node.label) } else { arm.label.clone() };
                    self.fragment(vec![(title, l, has)], "opt", "");
                }
                None
            }
        }
    }

    fn calls(&mut self, b: &Body, node: &FlowNode, me: &str, depth: u32) {
        self.calls_with(b, node, me, depth, false);
    }

    /// The messages of one statement, in evaluation order; calls made together by a
    /// concurrency combinator go in a `par` fragment.
    fn calls_with(&mut self, b: &Body, node: &FlowNode, me: &str, depth: u32, spawned: bool) {
        let sites = call_sites(node);
        let mut done_http = HashSet::new();
        let mut i = 0;
        while i < sites.len() {
            if let Some((group, _)) = sites[i].parallel {
                let mut branches: BTreeMap<usize, Vec<&CallSite>> = BTreeMap::new();
                let mut j = i;
                while j < sites.len() && sites[j].parallel.is_some_and(|(g, _)| g == group) {
                    branches.entry(sites[j].parallel.unwrap_or_default().1).or_default().push(&sites[j]);
                    j += 1;
                }
                let mut sections = Vec::new();
                for list in branches.values() {
                    let (l, has) = self.capture(|s| {
                        for site in list {
                            s.call(b, node, site, me, depth, spawned, &mut done_http);
                        }
                    });
                    sections.push((String::new(), l, has));
                }
                let kept: Vec<_> = sections.into_iter().filter(|s| s.2).collect();
                if kept.len() > 1 {
                    let mut kept = kept;
                    kept[0].0 = "in parallel".into();
                    self.fragment(kept, "par", "and");
                } else {
                    for (_, l, _) in kept {
                        // One branch: no fragment; lines were indented one level too deep.
                        self.lines.extend(l.into_iter().map(|x| x.strip_prefix("  ").map(str::to_owned).unwrap_or(x)));
                    }
                }
                i = j;
                continue;
            }
            self.call(b, node, &sites[i], me, depth, spawned, &mut done_http);
            i += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call(&mut self, b: &Body, node: &FlowNode, site: &CallSite, me: &str, depth: u32, spawned: bool, done_http: &mut HashSet<String>) {
        // Past the limit no new call is drawn, so every opened activation gets closed.
        if self.messages >= MAX_MESSAGES {
            self.truncated = true;
            return;
        }
        // `throw new X()` is the throw message itself.
        if site.construct && matches!(node.concept, Concept::Throw) {
            return;
        }
        let args = site.args.join(", ");
        // An HTTP request the knowledge graph matched to a route of the project.
        let http = self.index.graph.edges.iter().find(|e| {
            e.kind == StructEdgeKind::HttpCalls
                && e.from == b.sym.id
                && !done_http.contains(&e.to)
                && match &e.site {
                    Some(s) => within(s, &site.expr.span) || within(&site.expr.span, s),
                    None => within(&site.expr.span, &node.span),
                }
        });
        if let Some(e) = http {
            done_http.insert(e.to.clone());
            let Some(handler) = self.index.get(&e.to) else { return };
            if !self.opts.allows(e.confidence) {
                self.hidden += 1;
                return;
            }
            let route = self.input.knowledge.routes.iter().find(|r| r.handler == e.to);
            let text = route.map(|r| format!("{} {}", r.method, r.path)).unwrap_or_else(|| format!("HTTP {}", handler.name));
            let to = self.function_part(handler);
            let conf = e.confidence.or(Some(Confidence::Exact));
            self.message(me, &to, "->>+", &text, conf, "http", e.evidence.clone(), Some(&site.expr.span));
            let ret = if depth > 0 && !self.stack.contains(&e.to) { self.flow(&e.to, &to, me, depth - 1) } else { None };
            let reply = ret.map(|r| format!("HTTP response: {r}")).or(Some("HTTP response".into()));
            self.close(&to, me, reply);
            return;
        }
        let Some(callee) = self.resolver.resolve(b.sym, b.flow, node, site) else {
            if !self.opts.hide_noise {
                let to = self.part(&format!("ext:{}", site.name), "?", None, "external");
                self.message(me, &to, "->>", &format!("{}({args})", site.name), None, "call", None, Some(&site.expr.span));
            } else {
                self.hidden += 1;
            }
            return;
        };
        match callee {
            Callee::Project { target, confidence, evidence, via } => {
                if !self.opts.allows(Some(confidence)) {
                    self.hidden += 1;
                    return;
                }
                let Some(t) = self.index.get(&target) else { return };
                if t.kind.is_type() {
                    self.create(t, me, &args, confidence, evidence, site);
                    return;
                }
                if self.opts.hide_tests && self.index.is_test(&target) && !self.index.is_test(&b.sym.id) {
                    return;
                }
                let to = self.function_part(t);
                if let Some(iface) = via.as_deref().and_then(|v| self.index.get(v))
                    && self.notes.insert((to.clone(), iface.id.clone()))
                {
                    let how = if evidence.contains("provided by") { "dependency injection" } else { "only implementation" };
                    self.line(format!("Note right of {to}: via {} ({how})", seq_text(&iface.name, 40)));
                }
                let name = if t.kind == SymbolKind::Constructor { format!("new {}", self.index.owner_type(&t.id).map(|o| o.name.as_str()).unwrap_or("")) } else { t.name.clone() };
                let await_mark = if site.awaited { "await " } else { "" };
                // How often the callee ran in the recorded run.
                let ran = self.input.runtime.and_then(|r| r.calls.get(&target)).map(|n| format!(" ▶{n}")).unwrap_or_default();
                let text = format!("{await_mark}{name}({args}){ran}");
                let recursive = self.stack.contains(&target);
                let expand = depth > 0 && t.has_flow && !recursive && !spawned && !self.trivial(&target);
                if spawned {
                    self.message(me, &to, "-)", &text, Some(confidence), "async", Some(evidence), Some(&site.expr.span));
                } else if expand {
                    let open = self.lines.len();
                    self.message(me, &to, "->>+", &text, Some(confidence), "call", Some(evidence), Some(&site.expr.span));
                    let ret = self.flow(&target, &to, me, depth - 1);
                    if self.lines.len() == open + 1 {
                        // Nothing happened inside worth drawing: a plain message.
                        self.lines[open] = self.lines[open].replacen("->>+", "->>", 1);
                    } else {
                        self.close(&to, me, ret);
                    }
                } else {
                    let text = if recursive { format!("{text} ↻") } else { text };
                    self.message(me, &to, "->>", &text, Some(confidence), "call", Some(evidence), Some(&site.expr.span));
                }
            }
            Callee::External { owner, receiver, caps } => {
                let kind = if caps.contains(&"DB_READ") || caps.contains(&"DB_WRITE") || matches!(owner.as_str(), "DbSet" | "DbContext" | "IQueryable" | "PrismaClient" | "EntityManager" | "Repository") {
                    Some("database")
                } else if caps.contains(&"CACHE") {
                    Some("cache")
                } else if caps.contains(&"FILE_IO") {
                    Some("files")
                } else if caps.contains(&"LOCAL_STORAGE") {
                    Some("storage")
                } else if caps.contains(&"EVENT_EMIT") {
                    Some("events")
                } else {
                    None
                };
                match kind {
                    Some(k) => {
                        let to = if k == "database" { self.database(me) } else { self.infra(k) };
                        // `_db.Orders.FindAsync(id)` → `FindAsync(id) · Orders`.
                        let table = receiver.rsplit('.').next().filter(|t| t.chars().next().is_some_and(char::is_uppercase) && !receiver.ends_with(&owner));
                        let text = match table {
                            Some(t) if k == "database" && receiver.contains('.') => format!("{}({args}) · {t}", site.name),
                            _ => format!("{}({args})", site.name),
                        };
                        let why = format!("`{}` on `{receiver}` ({})", site.name, if caps.is_empty() { owner.clone() } else { caps.join(", ") });
                        let arrow = if spawned { "-)" } else { "->>" };
                        self.message(me, &to, arrow, &text, None, k, Some(why), Some(&site.expr.span));
                    }
                    None if !self.opts.hide_noise => {
                        let to = self.part(&format!("ext:{owner}"), &owner, None, "external");
                        self.message(me, &to, "->>", &format!("{}({args})", site.name), None, "call", None, Some(&site.expr.span));
                    }
                    None => self.hidden += 1,
                }
            }
        }
    }

    /// A body that calls, creates, throws and raises nothing (a getter, a setter, a field
    /// update): its call is one message, without an activation of its own.
    fn trivial(&self, id: &str) -> bool {
        let Some(flow) = self.input.flows.flow(id) else { return true };
        !flow.nodes.iter().any(|n| matches!(n.concept, Concept::Throw | Concept::Emit | Concept::Spawn | Concept::PropagateError) || !call_sites(n).is_empty())
    }

    /// `new X(...)` of a project type with behavior: the participant appears where it is created.
    fn create(&mut self, t: &Symbol, me: &str, args: &str, confidence: Confidence, evidence: String, site: &CallSite) {
        let has_behavior = self.index.children.get(t.id.as_str()).into_iter().flatten().any(|m| m.kind == SymbolKind::Method && m.has_flow);
        let is_model = self.index.models.contains_key(t.id.as_str());
        if self.opts.hide_noise && (!has_behavior || is_model) {
            self.hidden += 1;
            return;
        }
        let existed = self.by_key.contains_key(&t.id);
        let to = self.part(&t.id, &t.name, Some(t), "participant");
        if !existed {
            let i = self.by_key[&t.id];
            self.parts[i].created = true;
            self.line(format!("create participant {to} as {}", seq_text(&t.name, 40)));
        }
        self.message(me, &to, "->>", &format!("new {}({args})", t.name), Some(confidence), "create", Some(evidence), Some(&site.expr.span));
    }

    /// Participant declarations, grouped in boxes by build unit when there are several.
    fn header(&self, out: &mut String) {
        let declare = |p: &Part| {
            let kw = if p.kind == "actor" { "actor" } else { "participant" };
            format!("{kw} {} as {}", p.id, seq_text(&p.label, 40))
        };
        let units: Vec<&String> = {
            let mut seen = Vec::new();
            for p in self.parts.iter().filter(|p| !p.created) {
                if let Some(u) = &p.unit
                    && !seen.contains(&u)
                {
                    seen.push(u);
                }
            }
            seen
        };
        let boxed = units.len() > 1;
        // The actor first, then the boxes in order of appearance, then infrastructure.
        for p in self.parts.iter().filter(|p| !p.created && (p.kind == "actor" || p.id == "n0")) {
            out.push_str(&format!("  {}\n", declare(p)));
        }
        if boxed {
            for u in &units {
                out.push_str(&format!("  box rgba(139,152,168,0.08) {}\n", seq_text(u, 40)));
                for p in self.parts.iter().filter(|p| !p.created && p.id != "n0" && p.unit.as_ref() == Some(*u)) {
                    out.push_str(&format!("    {}\n", declare(p)));
                }
                out.push_str("  end\n");
            }
        }
        for p in self.parts.iter().filter(|p| !p.created && p.id != "n0" && p.kind != "actor" && (!boxed || p.unit.is_none())) {
            out.push_str(&format!("  {}\n", declare(p)));
        }
    }
}

/// Runtime sequence diagrams: the calls a recorded run made under one call, in order, with the
/// arguments, return values and exceptions it recorded. Identical consecutive calls (the same
/// callee making the same calls) are drawn once inside `loop N×`.
pub fn recorded(index: &Index, scoped: &Scoped, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut d = empty(UmlKind::Sequence);
    let Some(rt) = input.runtime.filter(|r| !r.spans.is_empty()) else {
        d.warnings.push("No recorded calls yet: run the program from Nodex and use it, or pick a call in the Timeline.".into());
        return d;
    };
    let mut children: HashMap<u32, Vec<&CallSpan>> = HashMap::new();
    for s in &rt.spans {
        if let Some(p) = s.parent.or(s.resumes).filter(|p| *p != s.id) {
            children.entry(p).or_default().push(s);
        }
    }
    for list in children.values_mut() {
        list.sort_by_key(|s| (s.start, s.id));
    }
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    fn size(id: u32, children: &HashMap<u32, Vec<&CallSpan>>, memo: &mut HashMap<u32, usize>, depth: usize) -> usize {
        if let Some(n) = memo.get(&id) {
            return *n;
        }
        let n = if depth > 200 { 0 } else { children.get(&id).map(|k| k.iter().map(|c| 1 + size(c.id, children, memo, depth + 1)).sum()).unwrap_or(0) };
        memo.insert(id, n);
        n
    }
    let ms = |s: &CallSpan| (s.end - s.start) as f64 / 1000.0;
    let mut cands: Vec<(&CallSpan, usize)> = rt.spans.iter().filter(|s| !(s.resumed && s.resumes.is_some()) && scoped.symbols.contains(&s.flow)).map(|s| (s, size(s.id, &children, &mut sizes, 0))).collect();
    // Calls whose caller is outside the scope first (where the scope is entered), then the
    // biggest, then the latest.
    let inside: HashSet<u32> = cands.iter().map(|(s, _)| s.id).collect();
    cands.sort_by(|a, b| {
        let ea = a.0.parent.is_some_and(|p| inside.contains(&p));
        let eb = b.0.parent.is_some_and(|p| inside.contains(&p));
        ea.cmp(&eb).then(b.1.cmp(&a.1)).then(b.0.start.cmp(&a.0.start))
    });
    cands.truncate(60);
    d.candidates = cands
        .iter()
        .map(|(s, n)| {
            let name = index.get(&s.flow).map(|x| function_label(index, x)).unwrap_or_else(|| s.flow.clone());
            UmlCandidate { id: format!("span:{}", s.id), label: format!("{name} #{}", s.id), detail: Some(format!("{:.1} ms · {n} calls under it", ms(s))), score: *n as f32 }
        })
        .collect();
    let by_id: HashMap<u32, &CallSpan> = rt.spans.iter().map(|s| (s.id, s)).collect();
    let root = opts.focus.as_deref().and_then(|f| f.strip_prefix("span:")).and_then(|n| n.parse::<u32>().ok()).and_then(|n| by_id.get(&n).copied()).or_else(|| cands.first().map(|(s, _)| *s));
    let Some(root) = root else {
        d.warnings.push("No recorded call of this scope yet: use the part of the program it covers, or choose a wider scope.".into());
        return d;
    };
    let mut s = Seq::new(index, input, opts);
    let sym = index.get(&root.flow);
    let actor = match (sym, root.parent) {
        (Some(x), None) => s.actor(x),
        _ => s.part("actor:", "Caller", None, "participant"),
    };
    let me = s.span_part(root);
    let text = s.span_text(root);
    s.message(&actor, &me, "->>+", &text, None, "call", Some(format!("recorded call #{} ({:.1} ms)", root.id, ms(root))), sym.and_then(|x| x.origins.first()));
    let mut sigs = HashMap::new();
    let limit = opts.depth.map(|d| d as usize).unwrap_or(usize::MAX);
    s.span_children(root, &me, &children, &mut sigs, 0, limit);
    s.span_close(root, &me, &actor);
    let compressed: usize = s.relations.iter().filter(|r| r.kind == "loop").map(|r| r.count as usize).sum();
    d.title = format!("Recorded sequence of {} #{}", sym.map(|x| function_label(index, x)).unwrap_or_else(|| root.flow.clone()), root.id);
    s.finish(&mut d);
    d.relations.retain(|r| r.kind != "loop");
    if compressed > 0 {
        d.warnings.push(format!("{compressed} repeated calls are drawn once, inside loop fragments."));
    }
    d
}

impl Seq<'_> {
    /// The participant of a recorded call: the function's owner, or the function itself when
    /// it is not in the project's graph.
    fn span_part(&mut self, s: &CallSpan) -> String {
        match self.index.get(&s.flow) {
            Some(f) => self.function_part(f),
            None => {
                let short = s.flow.rsplit(['.', ':', '/']).find(|p| !p.is_empty()).unwrap_or(&s.flow).to_owned();
                self.part(&format!("flow:{}", s.flow), &short, None, "participant")
            }
        }
    }

    fn span_text(&self, s: &CallSpan) -> String {
        let name = self.index.get(&s.flow).map(|f| f.name.split('(').next().unwrap_or(&f.name).to_owned()).unwrap_or_else(|| s.flow.rsplit(['.', ':']).next().unwrap_or(&s.flow).to_owned());
        format!("{name}({})", s.args.join(", "))
    }

    /// A call's shape: the callee and, recursively, what it called (equal shapes repeat).
    fn signature(s: &CallSpan, children: &HashMap<u32, Vec<&CallSpan>>, memo: &mut HashMap<u32, String>, depth: usize) -> String {
        if let Some(x) = memo.get(&s.id) {
            return x.clone();
        }
        let kids = if depth > 50 { String::new() } else { children.get(&s.id).map(|k| k.iter().map(|c| Self::signature(c, children, memo, depth + 1)).collect::<Vec<_>>().join(",")).unwrap_or_default() };
        let sig = format!("{}{}{}({kids})", s.flow, if s.threw { "!" } else { "" }, if s.resumed { "~" } else { "" });
        memo.insert(s.id, sig.clone());
        sig
    }

    fn span_children(&mut self, s: &CallSpan, me: &str, children: &HashMap<u32, Vec<&CallSpan>>, sigs: &mut HashMap<u32, String>, depth: usize, limit: usize) {
        let kids: Vec<&CallSpan> = children.get(&s.id).cloned().unwrap_or_default();
        if depth >= limit {
            if !kids.is_empty() {
                self.line(format!("Note over {me}: {} more calls (deeper than the depth)", kids.len()));
            }
            return;
        }
        let keys: Vec<String> = kids.iter().map(|k| Self::signature(k, children, sigs, 0)).collect();
        let mut i = 0;
        while i < kids.len() {
            if self.messages >= MAX_MESSAGES {
                self.truncated = true;
                return;
            }
            let mut j = i + 1;
            while j < kids.len() && keys[j] == keys[i] {
                j += 1;
            }
            let n = j - i;
            if n >= 2 {
                let times: Vec<f64> = kids[i..j].iter().map(|k| (k.end - k.start) as f64 / 1000.0).collect();
                let (lo, hi) = times.iter().fold((f64::MAX, 0.0f64), |(lo, hi), t| (lo.min(*t), hi.max(*t)));
                self.line(format!("loop {n}× ({lo:.1}–{hi:.1} ms)"));
                self.indent += 1;
                self.span_one(s, kids[i], me, children, sigs, depth, limit);
                self.indent -= 1;
                self.line("end".into());
                self.relations.push(UmlRelation { from: me.to_owned(), to: me.to_owned(), kind: "loop".into(), label: None, confidence: None, evidence: None, site: None, count: (n - 1) as u32 });
            } else {
                self.span_one(s, kids[i], me, children, sigs, depth, limit);
            }
            i = j;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn span_one(&mut self, parent: &CallSpan, k: &CallSpan, me: &str, children: &HashMap<u32, Vec<&CallSpan>>, sigs: &mut HashMap<u32, String>, depth: usize, limit: usize) {
        // A continuation of the caller itself after an `await`.
        if k.resumed && k.flow == parent.flow {
            self.line(format!("Note over {me}: resumed after a pause"));
            self.span_children(k, me, children, sigs, depth, limit);
            return;
        }
        let to = self.span_part(k);
        // Another function resuming (an iterator's next step, an async method's continuation)
        // is still a call into it.
        let text = if k.resumed { format!("{} ⟳", self.span_text(k)) } else { self.span_text(k) };
        let site = self.index.get(&k.flow).and_then(|f| f.origins.first().cloned());
        let evidence = format!("recorded call #{} at {:.1} ms, took {:.1} ms", k.id, k.start as f64 / 1000.0, (k.end - k.start) as f64 / 1000.0);
        self.message(me, &to, "->>+", &text, None, "call", Some(evidence), site.as_ref());
        self.span_children(k, &to, children, sigs, depth + 1, limit);
        self.span_close(k, &to, me);
    }

    fn span_close(&mut self, k: &CallSpan, me: &str, caller: &str) {
        if k.open {
            self.line(format!("Note over {me}: still running"));
        }
        // An exception raised here but kept by the call (an async method stores it in its task).
        if !k.threw && k.ret.is_none() && k.parent.is_some()
            && let Some(e) = &k.exception
        {
            self.line(format!("Note over {me}: ⚠ {}", seq_text(e, 60)));
        }
        if k.threw && me != caller {
            self.messages += 1;
            let what = k.exception.clone().unwrap_or_else(|| "exception".into());
            self.line(format!("{me}--x-{caller}: {}", seq_text(&what, 50)));
        } else {
            self.close(me, caller, k.ret.clone());
        }
    }
}
