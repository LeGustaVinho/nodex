//! Aggregates trace records into live frames and keeps a timeline for replay.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use ts_rs::TS;

use crate::map::{MethodMap, TraceMap};
use crate::protocol::{Record, RecordKind};
use crate::resolve::{Location, Resolver};

/// Timeline entries beyond this are not stored (aggregates keep updating).
pub const MAX_TIMELINE: usize = 2_000_000;

/// The node a thread is executing.
#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ActiveNode {
    pub thread: i32,
    pub flow: String,
    pub node: Option<String>,
}

/// Changes since the previous frame (or everything, for a snapshot).
#[derive(Serialize, TS, Clone, Debug, Default, PartialEq)]
#[ts(export)]
pub struct TraceFrame {
    /// Total records received.
    #[ts(type = "number")]
    pub events: u64,
    /// Records the program dropped because Nodex could not keep up.
    #[ts(type = "number")]
    pub dropped: u64,
    /// Visit counts: flow symbol → node id → count.
    #[ts(type = "Record<string, Record<string, number>>")]
    pub counts: BTreeMap<String, BTreeMap<String, u64>>,
    /// Times each function started: flow symbol → count.
    #[ts(type = "Record<string, number>")]
    pub calls: BTreeMap<String, u64>,
    /// Latest value written by a node: flow symbol → node id → `name = value`.
    pub values: BTreeMap<String, BTreeMap<String, String>>,
    /// Latest value of each variable: flow symbol → name → value.
    pub vars: BTreeMap<String, BTreeMap<String, String>>,
    /// What every thread is executing now.
    pub active: Vec<ActiveNode>,
    pub timeline_truncated: bool,
}

/// One recorded step of a flow, for replay.
#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct StepInfo {
    pub index: u32,
    #[ts(type = "number")]
    pub micros: i64,
    pub thread: i32,
    pub node: Option<String>,
    /// Values written while this step ran (`name = value`, `→ value` for returns).
    pub values: Vec<String>,
}

#[derive(Serialize, TS, Clone, Debug, Default, PartialEq)]
#[ts(export)]
pub struct StepsPage {
    pub total: u32,
    pub offset: u32,
    pub steps: Vec<StepInfo>,
}

enum Entry {
    Step { micros: i64, thread: i32, flow: u32, node: Option<u32> },
    Value { thread: i32, flow: u32, text: String },
}

/// A resolved point: interned flow, and the interned node when the point lies in a statement.
type Resolved = Option<(u32, Option<u32>)>;

struct Frame {
    method: i32,
    flow: Option<u32>,
    last_node: Option<u32>,
    args: Vec<String>,
}

#[derive(Default)]
struct Interner {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    fn id(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(s.to_owned());
        self.ids.insert(s.to_owned(), id);
        id
    }

    fn name(&self, id: u32) -> &str {
        &self.names[id as usize]
    }

    fn find(&self, s: &str) -> Option<u32> {
        self.ids.get(s).copied()
    }
}

pub struct Recorder {
    methods: HashMap<i32, MethodMap>,
    resolver: Resolver,
    points: HashMap<(i32, i32), Resolved>,
    method_flow: HashMap<i32, Option<u32>>,
    strings: Interner,
    counts: HashMap<(u32, u32), u64>,
    calls: HashMap<u32, u64>,
    values: HashMap<(u32, u32), String>,
    vars: HashMap<(u32, String), String>,
    changed_counts: HashSet<(u32, u32)>,
    changed_calls: HashSet<u32>,
    changed_values: HashSet<(u32, u32)>,
    changed_vars: HashSet<(u32, String)>,
    threads: HashMap<i32, Vec<Frame>>,
    active: HashMap<i32, (u32, Option<u32>)>,
    timeline: Vec<Entry>,
    events: u64,
    dropped: u64,
}

impl Recorder {
    pub fn new(map: &TraceMap, resolver: Resolver) -> Self {
        Self {
            methods: map.by_id(),
            resolver,
            points: HashMap::new(),
            method_flow: HashMap::new(),
            strings: Interner::default(),
            counts: HashMap::new(),
            calls: HashMap::new(),
            values: HashMap::new(),
            vars: HashMap::new(),
            changed_counts: HashSet::new(),
            changed_calls: HashSet::new(),
            changed_values: HashSet::new(),
            changed_vars: HashSet::new(),
            threads: HashMap::new(),
            active: HashMap::new(),
            timeline: Vec::new(),
            events: 0,
            dropped: 0,
        }
    }

    fn intern(&mut self, loc: Option<Location>) -> Resolved {
        let loc = loc?;
        let flow = self.strings.id(&loc.flow);
        let node = loc.node.map(|n| self.strings.id(&n));
        Some((flow, node))
    }

    fn point(&mut self, method: i32, point: i32) -> Resolved {
        if let Some(cached) = self.points.get(&(method, point)) {
            return *cached;
        }
        let loc = self
            .methods
            .get(&method)
            .and_then(|m| m.points.get(point as usize))
            .and_then(|p| self.resolver.locate(&p.file, p.start));
        let resolved = self.intern(loc);
        self.points.insert((method, point), resolved);
        resolved
    }

    /// The flow a method belongs to, through its first point.
    fn flow_of(&mut self, method: i32) -> Option<u32> {
        if let Some(cached) = self.method_flow.get(&method) {
            return *cached;
        }
        let flow = self.point(method, 0).map(|(f, _)| f);
        self.method_flow.insert(method, flow);
        flow
    }

    fn set_value(&mut self, flow: u32, node: u32, text: String) {
        self.values.insert((flow, node), text);
        self.changed_values.insert((flow, node));
    }

    pub fn record(&mut self, r: &Record) {
        self.events += 1;
        match r.kind {
            RecordKind::Enter => {
                let flow = self.flow_of(r.method);
                let is_resume = self.methods.get(&r.method).is_some_and(|m| m.kind == "resume");
                if let (Some(f), false) = (flow, is_resume) {
                    *self.calls.entry(f).or_default() += 1;
                    self.changed_calls.insert(f);
                }
                self.threads.entry(r.thread).or_default().push(Frame { method: r.method, flow, last_node: None, args: Vec::new() });
            }
            RecordKind::Step => {
                let Some((flow, node)) = self.point(r.method, r.arg) else { return };
                if let Some(n) = node {
                    *self.counts.entry((flow, n)).or_default() += 1;
                    self.changed_counts.insert((flow, n));
                }
                if let Some(frame) = self.threads.get_mut(&r.thread).and_then(|s| s.iter_mut().rev().find(|f| f.method == r.method))
                    && node.is_some() {
                        frame.last_node = node;
                    }
                self.active.insert(r.thread, (flow, node));
                if self.timeline.len() < MAX_TIMELINE {
                    self.timeline.push(Entry::Step { micros: r.micros, thread: r.thread, flow, node });
                }
            }
            RecordKind::Local | RecordKind::Arg | RecordKind::Return => {
                let Some(method) = self.methods.get(&r.method) else { return };
                let name = match r.kind {
                    RecordKind::Local => method.locals.get(&r.arg.to_string()).cloned(),
                    RecordKind::Arg => method.params.get(r.arg as usize).cloned(),
                    _ => None,
                };
                let value = r.text.clone().unwrap_or_default();
                let text = match &name {
                    Some(n) => format!("{n} = {value}"),
                    None => format!("→ {value}"),
                };
                let frame_info = self
                    .threads
                    .get_mut(&r.thread)
                    .and_then(|s| s.iter_mut().rev().find(|f| f.method == r.method))
                    .map(|f| {
                        if r.kind == RecordKind::Arg {
                            f.args.push(text.clone());
                        }
                        (f.flow, f.last_node, f.args.join(", "))
                    });
                let Some((Some(flow), last_node, args)) = frame_info else { return };
                if r.kind == RecordKind::Arg {
                    let entry = self.flow_name(flow).and_then(|f| self.resolver.entry_node(&f)).map(|e| self.strings.id(&e));
                    if let Some(e) = entry {
                        self.set_value(flow, e, args);
                    }
                } else if let Some(n) = last_node {
                    self.set_value(flow, n, text.clone());
                }
                if let Some(n) = name {
                    self.vars.insert((flow, n.clone()), value);
                    self.changed_vars.insert((flow, n));
                }
                if self.timeline.len() < MAX_TIMELINE {
                    self.timeline.push(Entry::Value { thread: r.thread, flow, text });
                }
                if r.kind == RecordKind::Return {
                    self.pop(r.thread, r.method);
                }
            }
            RecordKind::Exit => self.pop(r.thread, r.method),
            RecordKind::Dropped => self.dropped += r.arg.max(0) as u64,
            RecordKind::Unknown(_) => {}
        }
    }

    fn flow_name(&self, flow: u32) -> Option<String> {
        Some(self.strings.name(flow).to_owned())
    }

    fn pop(&mut self, thread: i32, method: i32) {
        let Some(stack) = self.threads.get_mut(&thread) else { return };
        if let Some(pos) = stack.iter().rposition(|f| f.method == method) {
            stack.truncate(pos);
        }
        // Back in the caller: show where it resumes.
        match stack.last() {
            Some(f) => {
                if let Some(flow) = f.flow {
                    self.active.insert(thread, (flow, f.last_node));
                }
            }
            None => {
                self.active.remove(&thread);
            }
        }
    }

    fn active_list(&self) -> Vec<ActiveNode> {
        let mut out: Vec<ActiveNode> = self
            .active
            .iter()
            .map(|(&thread, &(flow, node))| ActiveNode {
                thread,
                flow: self.strings.name(flow).to_owned(),
                node: node.map(|n| self.strings.name(n).to_owned()),
            })
            .collect();
        out.sort_by_key(|a| a.thread);
        out
    }

    /// Changes since the previous call.
    pub fn take_frame(&mut self) -> TraceFrame {
        let mut frame = TraceFrame {
            events: self.events,
            dropped: self.dropped,
            active: self.active_list(),
            timeline_truncated: self.timeline.len() >= MAX_TIMELINE,
            ..Default::default()
        };
        for (flow, node) in self.changed_counts.drain() {
            let count = self.counts[&(flow, node)];
            frame.counts.entry(self.strings.name(flow).to_owned()).or_default().insert(self.strings.name(node).to_owned(), count);
        }
        for flow in self.changed_calls.drain() {
            frame.calls.insert(self.strings.name(flow).to_owned(), self.calls[&flow]);
        }
        for (flow, node) in self.changed_values.drain() {
            let text = self.values[&(flow, node)].clone();
            frame.values.entry(self.strings.name(flow).to_owned()).or_default().insert(self.strings.name(node).to_owned(), text);
        }
        for (flow, name) in self.changed_vars.drain() {
            let value = self.vars[&(flow, name.clone())].clone();
            frame.vars.entry(self.strings.name(flow).to_owned()).or_default().insert(name, value);
        }
        frame
    }

    /// True when `take_frame` would report anything new.
    pub fn has_changes(&self) -> bool {
        !(self.changed_counts.is_empty() && self.changed_calls.is_empty() && self.changed_values.is_empty() && self.changed_vars.is_empty())
    }

    /// Everything recorded so far (for clients that connect late).
    pub fn snapshot(&self) -> TraceFrame {
        let mut frame = TraceFrame {
            events: self.events,
            dropped: self.dropped,
            active: self.active_list(),
            timeline_truncated: self.timeline.len() >= MAX_TIMELINE,
            ..Default::default()
        };
        for (&(flow, node), &count) in &self.counts {
            frame.counts.entry(self.strings.name(flow).to_owned()).or_default().insert(self.strings.name(node).to_owned(), count);
        }
        for (&flow, &count) in &self.calls {
            frame.calls.insert(self.strings.name(flow).to_owned(), count);
        }
        for ((flow, node), text) in &self.values {
            frame.values.entry(self.strings.name(*flow).to_owned()).or_default().insert(self.strings.name(*node).to_owned(), text.clone());
        }
        for ((flow, name), value) in &self.vars {
            frame.vars.entry(self.strings.name(*flow).to_owned()).or_default().insert(name.clone(), value.clone());
        }
        frame
    }

    /// Recorded steps of one flow, in execution order, with the values written at each step.
    pub fn steps(&self, flow: &str, offset: usize, limit: usize) -> StepsPage {
        let Some(flow_id) = self.strings.find(flow) else { return StepsPage::default() };
        let mut steps: Vec<StepInfo> = Vec::new();
        // Index into `steps` of each thread's latest step in this flow.
        let mut last: HashMap<i32, Option<usize>> = HashMap::new();
        for e in &self.timeline {
            match e {
                Entry::Step { micros, thread, flow, node } => {
                    if *flow == flow_id {
                        steps.push(StepInfo {
                            index: steps.len() as u32,
                            micros: *micros,
                            thread: *thread,
                            node: node.map(|n| self.strings.name(n).to_owned()),
                            values: Vec::new(),
                        });
                        last.insert(*thread, Some(steps.len() - 1));
                    } else {
                        last.insert(*thread, None);
                    }
                }
                Entry::Value { thread, flow, text } => {
                    if *flow == flow_id
                        && let Some(Some(i)) = last.get(thread) {
                            steps[*i].values.push(text.clone());
                        }
                }
            }
        }
        let total = steps.len() as u32;
        let steps = steps.into_iter().skip(offset).take(limit).collect();
        StepsPage { total, offset: offset as u32, steps }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MethodMap, PointMap, TraceMap};
    use nodex_ir::{Concept, FlowGraph, FlowNode, Language, Purity, Span};

    fn span(start: u32, end: u32) -> Span {
        Span { file: "A.cs".into(), start_byte: start, end_byte: end, ..Default::default() }
    }

    fn node(id: &str, concept: Concept, start: u32, end: u32) -> FlowNode {
        FlowNode {
            id: id.into(),
            concept,
            label: id.into(),
            code: id.into(),
            span: span(start, end),
            flavors: Vec::new(),
            purity: Purity::Impure,
            inputs: Vec::new(),
            outputs: Vec::new(),
            expr: None,
            writes: Vec::new(),
            calls: Vec::new(),
        }
    }

    fn setup() -> Recorder {
        let flow = FlowGraph {
            symbol: "S.M(int)".into(),
            name: "M".into(),
            language: Language::Csharp,
            span: span(0, 100),
            nodes: vec![node("entry", Concept::Entry, 0, 10), node("a", Concept::Var, 20, 30), node("b", Concept::Return, 40, 50)],
            edges: Vec::new(),
        };
        let map = TraceMap {
            version: 1,
            methods: vec![MethodMap {
                id: 1,
                name: "S::M".into(),
                kind: "method".into(),
                points: vec![
                    PointMap { file: "A.cs".into(), start: 12, end: 13 },
                    PointMap { file: "A.cs".into(), start: 20, end: 30 },
                    PointMap { file: "A.cs".into(), start: 40, end: 50 },
                ],
                locals: HashMap::from([("0".to_owned(), "x".to_owned())]),
                params: vec!["n".into()],
            }],
        };
        Recorder::new(&map, Resolver::new([&flow]))
    }

    fn rec(kind: RecordKind, arg: i32, text: Option<&str>) -> Record {
        Record { kind, thread: 1, micros: 0, method: 1, arg, text: text.map(str::to_owned) }
    }

    #[test]
    fn aggregates_counts_values_and_replay_steps() {
        let mut r = setup();
        for _ in 0..2 {
            r.record(&rec(RecordKind::Enter, 0, None));
            r.record(&rec(RecordKind::Arg, 0, Some("5")));
            r.record(&rec(RecordKind::Step, 0, None));
            r.record(&rec(RecordKind::Step, 1, None));
            r.record(&rec(RecordKind::Local, 0, Some("10")));
            r.record(&rec(RecordKind::Step, 2, None));
            r.record(&rec(RecordKind::Return, 0, Some("10")));
        }
        let frame = r.take_frame();
        assert_eq!(frame.calls["S.M(int)"], 2);
        assert_eq!(frame.counts["S.M(int)"]["a"], 2);
        assert_eq!(frame.values["S.M(int)"]["a"], "x = 10");
        assert_eq!(frame.values["S.M(int)"]["b"], "→ 10");
        assert_eq!(frame.values["S.M(int)"]["entry"], "n = 5");
        assert_eq!(frame.vars["S.M(int)"]["x"], "10");
        assert!(frame.active.is_empty(), "returned to no caller");
        assert!(!r.has_changes());

        let page = r.steps("S.M(int)", 0, 100);
        assert_eq!(page.total, 6);
        assert_eq!(page.steps[1].node.as_deref(), Some("a"));
        assert_eq!(page.steps[1].values, vec!["x = 10"]);
        assert_eq!(page.steps[0].node, None, "brace point maps to the flow only");
    }
}
