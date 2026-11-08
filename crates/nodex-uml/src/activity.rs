//! Activity diagrams: how one function runs, as a Mermaid flowchart.
//!
//! The function's flow is walked as nested blocks (see [`crate::flowtree`]): straight runs of
//! statements become actions (merged up to three lines by default), `if`/`switch` become
//! decisions whose branches meet again at the next action, loops a decision with a back edge,
//! `try` a box with dashed edges per caught exception type, `using`/`lock` a box, concurrency
//! combinators (`Task.WhenAll`, `Promise.all`, `join!`) a fork and a join bar. Actions are
//! annotated with what the knowledge graph knows (HTTP requests, persisted changes, who catches
//! a throw). With a depth, calls to project functions are expanded in place, each in a box named
//! after the function.

use std::collections::BTreeMap;

use nodex_ir::knowledge::Confidence;
use nodex_ir::{Concept, FlowGraph, FlowNode, Span, StructEdge, StructEdgeKind, Symbol};

use crate::calls::{Callee, Resolver};
use crate::flowtree::{Block, Step, call_sites, entry, thrown_type, tree, within};
use crate::index::Index;
use crate::mermaid::{class_defs, label, lines};
use crate::roots::{function_label, roots};
use crate::scope::Scoped;
use crate::{Granularity, UmlDiagram, UmlElement, UmlInput, UmlKind, UmlOptions, UmlRelation, UmlScope, empty};

const MAX_NODES: usize = 300;
const LINE: usize = 64;

/// Where control leaves a node: (node, edge label, dashed).
type Exit = (String, Option<String>, bool);

struct LoopCtx {
    head: Option<String>,
    breaks: Vec<Exit>,
}

/// What a `return` does: end the activity, or (in an expanded callee) go back to the caller.
struct RetCtx {
    collect: bool,
    returns: Vec<Exit>,
}

struct Body<'f> {
    flow: &'f FlowGraph,
    sym: &'f Symbol,
}

struct Act<'a> {
    index: &'a Index<'a>,
    resolver: Resolver<'a>,
    input: &'a UmlInput<'a>,
    opts: &'a UmlOptions,
    next: usize,
    groups: usize,
    lines: Vec<String>,
    indent: usize,
    edges: Vec<String>,
    classes: BTreeMap<&'static str, Vec<String>>,
    elements: Vec<UmlElement>,
    relations: Vec<UmlRelation>,
    final_id: Option<String>,
    error_id: Option<String>,
    range: Option<(String, u32, u32)>,
    truncated: bool,
    /// Functions being expanded (recursion guard) and the depth left.
    stack: Vec<String>,
    depth_left: u32,
    warnings: Vec<String>,
    inferred: usize,
}

pub fn build(index: &Index, scoped: &Scoped, scope: &UmlScope, input: &UmlInput, opts: &UmlOptions) -> UmlDiagram {
    let mut d = empty(UmlKind::Activity);
    let (candidates, chosen) = roots(index, scoped, scope, UmlKind::Activity, opts);
    d.candidates = candidates;
    let Some(root) = chosen else {
        d.warnings.push("No function with a body in this scope: choose a method, a function or code inside one.".into());
        return d;
    };
    let Some(flow) = input.flows.flow(&root) else {
        d.warnings.push(format!("The body of {root} is not available."));
        return d;
    };
    let Some(sym) = index.get(&root) else { return d };
    let range = match scope {
        UmlScope::Range { file, start_byte, end_byte } => Some((file.clone(), *start_byte, (*end_byte).max(*start_byte + 1))),
        _ => None,
    };
    let mut a = Act {
        index,
        resolver: Resolver::new(index),
        input,
        opts,
        next: 0,
        groups: 0,
        lines: Vec::new(),
        indent: 1,
        edges: Vec::new(),
        classes: BTreeMap::new(),
        elements: Vec::new(),
        relations: Vec::new(),
        final_id: None,
        error_id: None,
        range,
        truncated: false,
        stack: vec![root.clone()],
        depth_left: opts.depth.unwrap_or(0).min(4),
        warnings: Vec::new(),
        inferred: 0,
    };
    let f = Body { flow: &flow, sym };
    let start = a.new_id();
    a.lines.push(format!("  {start}((\" \"))"));
    a.classes.entry("start").or_default().push(start.clone());
    a.element(&start, sym, "start", "start", entry(&flow).map(|n| &n.span), None);
    let trigger = index
        .info
        .get(root.as_str())
        .and_then(|i| i.entry.as_ref())
        .map(|e| e.label.clone())
        .or_else(|| entry(&flow).map(|n| n.label.trim_start_matches("when ").trim_end_matches(" is called").to_owned()));
    let body = tree(&flow);
    let mut lc = LoopCtx { head: None, breaks: Vec::new() };
    let mut rc = RetCtx { collect: false, returns: Vec::new() };
    let exits = a.block(&f, &body, vec![(start, trigger, false)], &mut lc, &mut rc);
    if !exits.is_empty() {
        let fin = a.final_node(sym);
        a.connect(&exits, &fin);
    }

    let mut out = String::from("flowchart TD\n");
    for l in &a.lines {
        out.push_str(l);
        out.push('\n');
    }
    for e in &a.edges {
        out.push_str(&format!("  {e}\n"));
    }
    let used: Vec<&str> = a.classes.keys().copied().collect();
    class_defs(&mut out, &used);
    for (class, ids) in &a.classes {
        out.push_str(&format!("  class {} {class}\n", ids.join(",")));
    }
    d.mermaid = out;
    d.title = format!("Activity of {}", function_label(index, sym));
    d.elements = a.elements;
    d.relations = a.relations;
    d.truncated = a.truncated;
    d.warnings = a.warnings;
    if a.truncated {
        d.warnings.push(format!("Only the first {MAX_NODES} actions are drawn; choose a smaller function or code inside it."));
    }
    if a.classes.contains_key("hot") {
        d.warnings.push("Green actions ran in the recorded run; dimmed ones never did.".into());
    }
    if a.inferred > 0 && opts.depth.unwrap_or(0) > 0 {
        d.warnings.push(format!("{} expanded calls were resolved from declared types, not by a compiler.", a.inferred));
    }
    d
}

/// Mermaid shapes used here.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Action,
    Decision,
    Signal,
    Spawn,
    Bar,
}

impl<'a> Act<'a> {
    fn new_id(&mut self) -> String {
        let id = format!("n{}", self.next);
        self.next += 1;
        id
    }

    fn pad(&self) -> String {
        "  ".repeat(self.indent)
    }

    fn element(&mut self, id: &str, sym: &Symbol, text: &str, kind: &str, span: Option<&Span>, detail: Option<String>) {
        self.elements.push(UmlElement {
            mermaid_id: id.to_owned(),
            symbol: Some(sym.id.clone()),
            label: text.to_owned(),
            kind: kind.to_owned(),
            span: span.cloned(),
            context: false,
            detail,
        });
    }

    fn node(&mut self, f: &Body, shape: Shape, parts: &[String], node: Option<&FlowNode>, kind: &str) -> String {
        let id = self.new_id();
        let text = lines(parts, LINE);
        let decl = match shape {
            Shape::Action => format!("{id}(\"{text}\")"),
            Shape::Decision => format!("{id}{{\"{text}\"}}"),
            Shape::Signal => format!("{id}>\"{text}\"]"),
            Shape::Spawn => format!("{id}[/\"{text}\"/]"),
            Shape::Bar => format!("{id}[\" \"]"),
        };
        self.lines.push(format!("{}{decl}", self.pad()));
        if shape == Shape::Bar {
            self.classes.entry("bar").or_default().push(id.clone());
        }
        if let (Some(n), Some((file, start, end))) = (node, &self.range)
            && n.span.file == *file
            && n.span.start_byte < *end
            && *start < n.span.end_byte
        {
            self.classes.entry("hl").or_default().push(id.clone());
        }
        // A recorded run: what ran (and how often), what never did although the function ran.
        let mut detail = node.map(|n| n.code.clone());
        if let (Some(n), Some(rt)) = (node, self.input.runtime) {
            let ran = rt.counts.get(&f.flow.symbol).and_then(|c| c.get(&n.id)).copied().unwrap_or(0);
            if ran > 0 {
                self.classes.entry("hot").or_default().push(id.clone());
                detail = Some(format!("{} · ran {ran}×", detail.unwrap_or_default()));
            } else if rt.calls.get(&f.flow.symbol).is_some_and(|c| *c > 0) && shape != Shape::Bar {
                self.classes.entry("cold").or_default().push(id.clone());
                detail = Some(format!("{} · never ran in the recorded run", detail.unwrap_or_default()));
            }
        }
        let plain = parts.join(" · ");
        self.element(&id, f.sym, &plain, kind, node.map(|n| &n.span), detail);
        id
    }

    fn connect(&mut self, from: &[Exit], to: &str) {
        for (src, text, dashed) in from {
            let arrow = if *dashed { "-.->" } else { "-->" };
            match text.as_deref().filter(|t| !t.is_empty()) {
                Some(t) => self.edges.push(format!("{src} {arrow}|\"{}\"| {to}", label(t, 60))),
                None => self.edges.push(format!("{src} {arrow} {to}")),
            }
        }
    }

    fn final_node(&mut self, sym: &Symbol) -> String {
        if let Some(id) = &self.final_id {
            return id.clone();
        }
        let id = self.new_id();
        self.lines.push(format!("  {id}(((\" \")))"));
        self.classes.entry("final").or_default().push(id.clone());
        self.element(&id, sym, "end", "end", None, None);
        self.final_id = Some(id.clone());
        id
    }

    fn error_node(&mut self, sym: &Symbol) -> String {
        if let Some(id) = &self.error_id {
            return id.clone();
        }
        let id = self.new_id();
        self.lines.push(format!("  {id}(((\"✕\")))"));
        self.classes.entry("err").or_default().push(id.clone());
        self.element(&id, sym, "ends with an error", "error-end", None, None);
        self.error_id = Some(id.clone());
        id
    }

    fn open_group(&mut self, title: &str) -> String {
        let id = format!("g{}", self.groups);
        self.groups += 1;
        self.lines.push(format!("{}subgraph {id} [\"{}\"]", self.pad(), label(title, 60)));
        self.indent += 1;
        id
    }

    fn close_group(&mut self) {
        self.indent -= 1;
        self.lines.push(format!("{}end", self.pad()));
    }

    /// Knowledge relations of the function that show at this statement (an HTTP request, a
    /// write to an entity, a throw).
    fn facts_at<'g>(&'g self, f: &Body, node: &FlowNode, kind: StructEdgeKind) -> Vec<&'g StructEdge> {
        self.index
            .graph
            .edges
            .iter()
            .filter(|e| e.kind == kind && e.from == f.sym.id)
            .filter(|e| match &e.site {
                Some(site) => within(site, &node.span) || within(&node.span, site),
                // Without a site, the evidence quotes the code (`writes `_db.Orders``).
                None => e.evidence.as_deref().and_then(|ev| ev.split('`').nth(1)).is_some_and(|q| !q.is_empty() && node.code.contains(q)),
            })
            .collect()
    }

    fn step_lines(&self, f: &Body, node: &FlowNode) -> Vec<String> {
        let base = if self.opts.code_labels { node.code.clone() } else { node.label.clone() };
        let prefix = match node.concept {
            Concept::Await => "⏳ ",
            Concept::Yield => "⇢ ",
            Concept::Wait => "⏸ ",
            _ => "",
        };
        let mut out = vec![format!("{prefix}{base}")];
        for e in self.facts_at(f, node, StructEdgeKind::HttpCalls) {
            let route = self.input.knowledge.routes.iter().find(|r| r.handler == e.to);
            out.push(match route {
                Some(r) => format!("→ HTTP {} {}", r.method, r.path),
                None => format!("→ HTTP {}", self.index.get(&e.to).map(|s| s.name.as_str()).unwrap_or("request")),
            });
        }
        for e in self.facts_at(f, node, StructEdgeKind::Mutates) {
            out.push(format!("✎ {}", self.index.get(&e.to).map(|s| s.name.as_str()).unwrap_or("data")));
        }
        out
    }

    /// Project functions this statement calls whose body can be expanded.
    fn expandable(&self, f: &Body, node: &FlowNode) -> Vec<(String, Callee)> {
        if self.depth_left == 0 {
            return Vec::new();
        }
        call_sites(node)
            .iter()
            .filter(|s| !s.construct)
            .filter_map(|s| self.resolver.resolve(f.sym, f.flow, node, s))
            .filter_map(|c| match &c {
                Callee::Project { target, .. } if self.index.get(target).is_some_and(|t| t.has_flow) && !self.stack.contains(target) => Some((target.clone(), c)),
                _ => None,
            })
            .collect()
    }

    fn plain(step: &Step) -> bool {
        step.arms.is_empty()
            && !matches!(
                step.node.concept,
                Concept::Return | Concept::Throw | Concept::PropagateError | Concept::Break | Concept::Continue | Concept::Jump | Concept::Spawn | Concept::Emit
            )
            && !call_sites(step.node).iter().any(|s| s.parallel.is_some())
    }

    fn block(&mut self, f: &Body, block: &Block, mut cur: Vec<Exit>, lc: &mut LoopCtx, rc: &mut RetCtx) -> Vec<Exit> {
        let max_run = match self.opts.granularity {
            Granularity::Statement => 1,
            Granularity::Grouped => 3,
            Granularity::Decisions => usize::MAX,
        };
        let mut i = 0;
        while i < block.steps.len() {
            if cur.is_empty() {
                // Unreachable code after a return or throw is not drawn.
                break;
            }
            if self.next >= MAX_NODES {
                self.truncated = true;
                let more = self.node(f, Shape::Action, &[format!("… {} more statements", block.steps.len() - i)], None, "more");
                self.connect(&cur, &more);
                return vec![(more, None, false)];
            }
            let step = &block.steps[i];
            if Self::plain(step) && self.expandable(f, step.node).is_empty() {
                // A straight run of statements.
                let mut run = vec![step];
                while run.len() < max_run
                    && let Some(next) = block.steps.get(i + run.len())
                    && Self::plain(next)
                    && self.expandable(f, next.node).is_empty()
                {
                    run.push(next);
                }
                let mut parts = Vec::new();
                if self.opts.granularity == Granularity::Decisions && run.len() > 1 {
                    parts.extend(self.step_lines(f, run[0].node));
                    parts.push(format!("… {} more steps", run.len() - 1));
                } else {
                    for s in &run {
                        parts.extend(self.step_lines(f, s.node));
                    }
                }
                // The selection is marked when any merged statement is in it.
                let marked = run.iter().find(|s| self.in_range(s.node)).map(|s| s.node).unwrap_or(run[0].node);
                let id = self.node(f, Shape::Action, &parts, Some(marked), "action");
                if let Some(last) = self.elements.last_mut() {
                    let first = &run[0].node.span;
                    let end = &run[run.len() - 1].node.span;
                    last.span = Some(Span { end_byte: end.end_byte, end_line: end.end_line, end_col: end.end_col, ..first.clone() });
                }
                self.connect(&cur, &id);
                cur = vec![(id, None, false)];
                i += run.len();
                continue;
            }
            cur = self.step(f, step, cur, lc, rc);
            i += 1;
        }
        cur
    }

    fn in_range(&self, n: &FlowNode) -> bool {
        self.range.as_ref().is_some_and(|(file, start, end)| n.span.file == *file && n.span.start_byte < *end && *start < n.span.end_byte)
    }

    fn arm_label(step: &Step, pin: &str, text: &str) -> Option<String> {
        let l = match pin {
            "true" => "yes".to_owned(),
            "false" => "no".to_owned(),
            "body" if step.node.concept == Concept::LoopForEach => "each".to_owned(),
            "body" => "repeat".to_owned(),
            "else" => "otherwise".to_owned(),
            _ => text.trim_end_matches(':').to_owned(),
        };
        (!l.is_empty()).then_some(l)
    }

    fn step(&mut self, f: &Body, step: &Step, cur: Vec<Exit>, lc: &mut LoopCtx, rc: &mut RetCtx) -> Vec<Exit> {
        let node = step.node;
        match node.concept {
            Concept::Branch | Concept::Match | Concept::BuildSwitch => {
                let text = match node.concept {
                    Concept::Branch => {
                        let cond = if self.opts.code_labels { node.code.clone() } else { node.label.trim_start_matches("if ").to_owned() };
                        format!("{cond}?")
                    }
                    _ if self.opts.code_labels => node.code.clone(),
                    _ => node.label.clone(),
                };
                let dec = self.node(f, Shape::Decision, &[text], Some(node), "decision");
                self.connect(&cur, &dec);
                let mut exits = Vec::new();
                for arm in &step.arms {
                    let entry = vec![(dec.clone(), Self::arm_label(step, &arm.pin, &arm.label), false)];
                    if arm.block.is_empty() {
                        exits.extend(entry);
                    } else {
                        exits.extend(self.block(f, &arm.block, entry, lc, rc));
                    }
                }
                if node.concept == Concept::Match && !step.arms.iter().any(|a| a.label.contains("default") || a.label.trim() == "_" || a.label.contains("_ =>")) {
                    // No default case: the value can match nothing and go on.
                    exits.push((dec, Some("no case".into()), false));
                }
                exits
            }
            Concept::LoopWhile | Concept::LoopRange | Concept::LoopForEach | Concept::LoopInfinite => {
                let head = self.node(f, Shape::Decision, &[if self.opts.code_labels { node.code.clone() } else { node.label.clone() }], Some(node), "loop");
                self.connect(&cur, &head);
                let mut inner = LoopCtx { head: Some(head.clone()), breaks: Vec::new() };
                if let Some(body) = step.arm("body") {
                    let entry = vec![(head.clone(), Self::arm_label(step, "body", &body.label), false)];
                    let back = if body.block.is_empty() { entry } else { self.block(f, &body.block, entry, &mut inner, rc) };
                    self.connect(&back, &head);
                }
                let mut exits = inner.breaks;
                if node.concept != Concept::LoopInfinite {
                    exits.insert(0, (head, Some("done".into()), false));
                }
                exits
            }
            Concept::Break => match lc.head {
                Some(_) => {
                    lc.breaks.extend(cur);
                    Vec::new()
                }
                None => cur,
            },
            Concept::Continue => {
                if let Some(head) = lc.head.clone() {
                    let cur: Vec<Exit> = cur.into_iter().map(|(n, l, d)| (n, l.or(Some("next".into())), d)).collect();
                    self.connect(&cur, &head);
                    Vec::new()
                } else {
                    cur
                }
            }
            Concept::Return => {
                let bare = matches!(node.label.trim(), "return" | "return;" | "") || node.flavors.iter().any(|f| f == "implicit");
                let exits = if bare {
                    cur
                } else {
                    let parts = self.step_lines(f, node);
                    let id = self.node(f, Shape::Action, &parts, Some(node), "return");
                    self.connect(&cur, &id);
                    vec![(id, None, false)]
                };
                if rc.collect {
                    rc.returns.extend(exits);
                } else {
                    let fin = self.final_node(f.sym);
                    self.connect(&exits, &fin);
                }
                Vec::new()
            }
            Concept::Throw | Concept::PropagateError => {
                let mut parts = self.step_lines(f, node);
                let ty = thrown_type(node);
                let caught: Vec<String> = self
                    .index
                    .graph
                    .edges
                    .iter()
                    .filter(|e| e.kind == StructEdgeKind::CaughtBy && e.from == f.sym.id)
                    .filter_map(|e| self.index.get(&e.to).map(|s| s.name.clone()))
                    .collect();
                if !caught.is_empty() {
                    parts.push(format!("↯ caught by {}", caught.join(", ")));
                }
                let id = self.node(f, Shape::Action, &parts, Some(node), "throw");
                self.classes.entry("err").or_default().push(id.clone());
                self.connect(&cur, &id);
                let end = self.error_node(f.sym);
                self.connect(&[(id, ty, false)], &end);
                Vec::new()
            }
            Concept::Jump => {
                let parts = self.step_lines(f, node);
                let id = self.node(f, Shape::Action, &parts, Some(node), "jump");
                self.connect(&cur, &id);
                let fin = self.final_node(f.sym);
                self.connect(&[(id, Some("jump".into()), true)], &fin);
                Vec::new()
            }
            Concept::Try => {
                let mut exits = cur.clone();
                let mut group = None;
                if let Some(body) = step.arm("try").filter(|a| !a.block.is_empty()) {
                    let g = self.open_group("try");
                    exits = self.block(f, &body.block, cur, lc, rc);
                    self.close_group();
                    group = Some(g);
                }
                for arm in step.arms.iter().filter(|a| a.pin.starts_with("catch:")) {
                    let Some(g) = &group else { break };
                    let what = match arm.ty.as_deref() {
                        Some("*") | None => "any error".to_owned(),
                        Some(t) => t.to_owned(),
                    };
                    let entry = vec![(g.clone(), Some(format!("catch {what}")), true)];
                    if arm.block.is_empty() {
                        exits.extend(entry);
                    } else {
                        exits.extend(self.block(f, &arm.block, entry, lc, rc));
                    }
                }
                if let Some(fin) = step.arm("finally").filter(|a| !a.block.is_empty()) {
                    let g = self.open_group("finally");
                    // `finally` runs on every path; drawn once, after the normal ones.
                    exits = self.block(f, &fin.block, exits, lc, rc);
                    self.close_group();
                    let _ = g;
                }
                exits
            }
            Concept::Scope | Concept::CriticalSection => {
                let title = if self.opts.code_labels { node.code.clone() } else { node.label.clone() };
                let Some(body) = step.arm("body").filter(|a| !a.block.is_empty()) else { return cur };
                self.open_group(&title);
                let exits = self.block(f, &body.block, cur, lc, rc);
                self.close_group();
                exits
            }
            _ => self.simple(f, step, cur, lc, rc),
        }
    }

    /// A statement that is not a control structure: an action (or a signal, a spawn), with a
    /// fork/join for concurrency combinators, expanded callees, and side exits (`else`,
    /// `error`).
    fn simple(&mut self, f: &Body, step: &Step, mut cur: Vec<Exit>, lc: &mut LoopCtx, rc: &mut RetCtx) -> Vec<Exit> {
        let node = step.node;
        // Fork and join around the operations a combinator runs together.
        let sites = call_sites(node);
        let mut branches: BTreeMap<(usize, usize), Vec<String>> = BTreeMap::new();
        for s in sites.iter().filter(|s| s.parallel.is_some()) {
            branches.entry(s.parallel.unwrap_or_default()).or_default().push(format!("{}()", s.name));
        }
        if branches.len() >= 2 {
            let fork = self.node(f, Shape::Bar, &[], None, "fork");
            self.connect(&cur, &fork);
            let join = self.new_id();
            let mut ends = Vec::new();
            for calls in branches.values() {
                let id = self.node(f, Shape::Action, calls, Some(node), "action");
                self.connect(&[(fork.clone(), None, false)], &id);
                ends.push((id, None, false));
            }
            self.lines.push(format!("{}{join}[\" \"]", self.pad()));
            self.classes.entry("bar").or_default().push(join.clone());
            self.element(&join, f.sym, "join", "join", None, None);
            self.connect(&ends, &join);
            cur = vec![(join, None, false)];
        }
        let shape = match node.concept {
            Concept::Emit => Shape::Signal,
            Concept::Spawn => Shape::Spawn,
            _ => Shape::Action,
        };
        let parts = self.step_lines(f, node);
        let id = self.node(f, shape, &parts, Some(node), "action");
        self.connect(&cur, &id);
        let mut exits = if step.falls { vec![(id.clone(), None, false)] } else { Vec::new() };
        for arm in &step.arms {
            match arm.pin.as_str() {
                // Rust `?`: the error goes back to the caller.
                "error" => {
                    let end = self.error_node(f.sym);
                    self.connect(&[(id.clone(), Some("Err / None".into()), true)], &end);
                }
                _ => {
                    let entry = vec![(id.clone(), Self::arm_label(step, &arm.pin, &arm.label), false)];
                    if arm.block.is_empty() {
                        exits.extend(entry);
                    } else {
                        exits.extend(self.block(f, &arm.block, entry, lc, rc));
                    }
                }
            }
        }
        // Calls expanded in place.
        for (target, callee) in self.expandable(f, node) {
            if exits.is_empty() {
                break;
            }
            exits = self.expand(f, &id, &target, &callee, exits);
        }
        exits
    }

    fn expand(&mut self, from: &Body, at: &str, target: &str, callee: &Callee, cur: Vec<Exit>) -> Vec<Exit> {
        let Some(sym) = self.index.get(target) else { return cur };
        let Some(flow) = self.input.flows.flow(target) else { return cur };
        let Callee::Project { confidence, evidence, .. } = callee else { return cur };
        if *confidence != Confidence::Exact {
            self.inferred += 1;
        }
        let title = function_label(self.index, sym);
        let g = self.open_group(&title);
        let detail = Some(format!("{title}, expanded in place"));
        self.element(&g, sym, &title, "call", sym.origins.first(), detail);
        self.relations.push(UmlRelation {
            from: at.to_owned(),
            to: g.clone(),
            kind: "call".into(),
            label: Some(title.clone()),
            confidence: (*confidence != Confidence::Exact).then_some(*confidence),
            evidence: Some(evidence.clone()),
            site: None,
            count: 1,
        });
        self.stack.push(target.to_owned());
        self.depth_left -= 1;
        let f = Body { flow: &flow, sym };
        let body = tree(&flow);
        let mut lc = LoopCtx { head: None, breaks: Vec::new() };
        let mut rc = RetCtx { collect: true, returns: Vec::new() };
        let entry: Vec<Exit> = cur.into_iter().map(|(n, l, d)| (n, l.or(Some(format!("call {}", sym.name))), d)).collect();
        let mut exits = self.block(&f, &body, entry, &mut lc, &mut rc);
        exits.extend(rc.returns);
        self.depth_left += 1;
        self.stack.pop();
        self.close_group();
        let _ = from;
        exits
    }
}
