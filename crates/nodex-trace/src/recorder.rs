//! Aggregates trace records into live frames and keeps a timeline for replay.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use ts_rs::TS;

use crate::map::{MethodMap, TraceMap};
use crate::protocol::{Record, RecordKind};
use crate::resolve::{Location, Resolver};

/// Timeline entries beyond this are not stored (aggregates keep updating).
pub const MAX_TIMELINE: usize = 2_000_000;
/// Call spans beyond this are not stored.
pub const MAX_SPANS: usize = 500_000;
/// Longest argument, return or exception text kept on a call span.
pub const MAX_SPAN_TEXT: usize = 120;
/// Arguments kept per call span.
pub const MAX_SPAN_ARGS: usize = 6;

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
    /// Exceptions thrown while a node ran: flow symbol → node id → count.
    #[ts(type = "Record<string, Record<string, number>>")]
    pub throws: BTreeMap<String, BTreeMap<String, u64>>,
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

/// One execution of a function on a thread (for the flame chart).
#[derive(Serialize, TS, Clone, Debug, PartialEq, Default)]
#[ts(export)]
pub struct CallSpan {
    pub thread: i32,
    pub flow: String,
    #[ts(type = "number")]
    pub start: i64,
    #[ts(type = "number")]
    pub end: i64,
    /// Nesting depth on its thread (0 = outermost traced call).
    pub depth: u32,
    /// An async method or iterator resuming after a pause, not a fresh call.
    pub resumed: bool,
    /// The call ended because an exception left it.
    pub threw: bool,
    /// Still running when the recording was read.
    pub open: bool,
    /// Identifier of this execution, unique and stable within a run (assigned when it starts).
    pub id: u32,
    /// The execution that was running on the same thread when this one started (its caller),
    /// `None` for an outermost call. Refers to [`CallSpan::id`] even when that span is not in
    /// the same page.
    pub parent: Option<u32>,
    /// Flow node of the caller that was executing when this call started (the call statement),
    /// when the caller's position is known.
    pub call_site: Option<String>,
    /// For a resumed async method or iterator (`resumed`), the most recent fresh call of the same
    /// function: the invocation this continuation belongs to.
    pub resumes: Option<u32>,
    /// Argument values as `name = value`, shortened.
    pub args: Vec<String>,
    /// Returned value, shortened.
    pub ret: Option<String>,
    /// Exception message when an exception was thrown while this call ran, shortened.
    pub exception: Option<String>,
}

#[derive(Serialize, TS, Clone, Debug, Default, PartialEq)]
#[ts(export)]
pub struct CallsPage {
    pub total: u32,
    #[ts(type = "number")]
    pub start: i64,
    #[ts(type = "number")]
    pub end: i64,
    pub spans: Vec<CallSpan>,
}

struct SpanRec {
    thread: i32,
    flow: u32,
    start: i64,
    end: i64,
    depth: u32,
    resumed: bool,
    threw: bool,
    calling: Calling,
}

/// Who started an execution and what it carried, kept from `Enter` to the span.
#[derive(Clone, Default)]
struct Calling {
    id: u32,
    parent: Option<u32>,
    call_site: Option<u32>,
    resumes: Option<u32>,
    args: Vec<String>,
    ret: Option<String>,
    exception: Option<String>,
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
    start: i64,
    resumed: bool,
    calling: Calling,
}

/// Shortens a value shown on a call span.
fn short(text: &str) -> String {
    if text.chars().count() <= MAX_SPAN_TEXT {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MAX_SPAN_TEXT - 1).collect();
    out.push('…');
    out
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
    throws: HashMap<(u32, u32), u64>,
    changed_throws: HashSet<(u32, u32)>,
    spans: Vec<SpanRec>,
    first_micros: Option<i64>,
    last_micros: i64,
    threads: HashMap<i32, Vec<Frame>>,
    active: HashMap<i32, (u32, Option<u32>)>,
    timeline: Vec<Entry>,
    events: u64,
    dropped: u64,
    /// Highest frame number received per process (see [`RecordKind::Sequence`]).
    sequences: HashMap<i32, i32>,
    /// Next [`CallSpan::id`].
    next_call: u32,
    /// Latest fresh (not resumed) call of each flow, for [`CallSpan::resumes`].
    last_call: HashMap<u32, u32>,
}

impl Recorder {
    /// Adds methods of files instrumented again during the run (a browser app under a dev
    /// server) and resolves against the current IR from now on.
    pub fn extend(&mut self, methods: Vec<crate::map::MethodMap>, resolver: Resolver) {
        for m in methods {
            self.methods.insert(m.id, m);
        }
        self.resolver = resolver;
        self.points.clear();
        self.method_flow.clear();
    }

    /// The highest method id known (new methods continue after it).
    pub fn max_method_id(&self) -> i32 {
        self.methods.keys().copied().max().unwrap_or(0)
    }

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
            throws: HashMap::new(),
            changed_throws: HashSet::new(),
            spans: Vec::new(),
            first_micros: None,
            last_micros: 0,
            threads: HashMap::new(),
            active: HashMap::new(),
            timeline: Vec::new(),
            events: 0,
            dropped: 0,
            sequences: HashMap::new(),
            next_call: 0,
            last_call: HashMap::new(),
        }
    }

    /// The highest frame number received from process `pid` (0 when none).
    pub fn last_sequence(&self, pid: i32) -> i32 {
        self.sequences.get(&pid).copied().unwrap_or(0)
    }

    /// Records the frames of a crash tail file that did not already arrive over the socket.
    pub fn replay_tail(&mut self, records: &[Record]) -> usize {
        let mut skip = false;
        let mut replayed = 0;
        for r in records {
            if r.kind == RecordKind::Sequence {
                skip = r.arg <= self.last_sequence(r.thread);
            }
            if !skip {
                self.record(r);
                replayed += 1;
            }
        }
        replayed
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
        if r.kind == RecordKind::Sequence {
            let last = self.sequences.entry(r.thread).or_default();
            *last = (*last).max(r.arg);
            return;
        }
        self.events += 1;
        if r.kind != RecordKind::Dropped {
            self.first_micros.get_or_insert(r.micros);
            self.last_micros = self.last_micros.max(r.micros);
        }
        match r.kind {
            RecordKind::Enter => {
                let flow = self.flow_of(r.method);
                let is_resume = self.methods.get(&r.method).is_some_and(|m| m.kind == "resume");
                if let (Some(f), false) = (flow, is_resume) {
                    *self.calls.entry(f).or_default() += 1;
                    self.changed_calls.insert(f);
                }
                // The caller is whatever runs on this thread now. A resumed async method or
                // iterator is started by the scheduler (often nothing traced), so it also
                // records the latest fresh call of its function: the invocation it continues.
                let id = self.next_call;
                self.next_call += 1;
                let caller = self.threads.get(&r.thread).and_then(|s| s.last());
                let calling = Calling {
                    id,
                    parent: caller.map(|c| c.calling.id),
                    call_site: caller.and_then(|c| c.last_node),
                    resumes: if is_resume { flow.and_then(|f| self.last_call.get(&f).copied()) } else { None },
                    ..Calling::default()
                };
                if let (Some(f), false) = (flow, is_resume) {
                    self.last_call.insert(f, id);
                }
                self.threads.entry(r.thread).or_default().push(Frame {
                    method: r.method,
                    flow,
                    last_node: None,
                    args: Vec::new(),
                    start: r.micros,
                    resumed: is_resume,
                    calling,
                });
            }
            RecordKind::Step => {
                // A step in a caller means the frames above it were unwound by an exception.
                let unwound = self.threads.get(&r.thread).and_then(|s| {
                    let top = s.last()?;
                    if top.method == r.method {
                        return None;
                    }
                    s.iter().rposition(|f| f.method == r.method)
                });
                if let Some(pos) = unwound {
                    self.close_frames(r.thread, pos + 1, r.micros, true);
                }
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
                            if f.calling.args.len() < MAX_SPAN_ARGS {
                                f.calling.args.push(short(&text));
                            }
                        }
                        if r.kind == RecordKind::Return {
                            f.calling.ret = Some(short(&value));
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
                    self.pop(r.thread, r.method, r.micros);
                }
            }
            RecordKind::Exit => self.pop(r.thread, r.method, r.micros),
            RecordKind::Throw => {
                if let Some(top) = self.threads.get_mut(&r.thread).and_then(|s| s.last_mut()) {
                    top.calling.exception = Some(short(r.text.as_deref().unwrap_or("exception")));
                }
                let current = self.threads.get(&r.thread).and_then(|s| s.last()).map(|f| (f.flow, f.last_node));
                let Some((Some(flow), Some(node))) = current else { return };
                *self.throws.entry((flow, node)).or_default() += 1;
                self.changed_throws.insert((flow, node));
                let text = format!("⚠ {}", r.text.clone().unwrap_or_else(|| "exception".into()));
                self.set_value(flow, node, text.clone());
                if self.timeline.len() < MAX_TIMELINE {
                    self.timeline.push(Entry::Value { thread: r.thread, flow, text });
                }
            }
            RecordKind::Dropped => self.dropped += r.arg.max(0) as u64,
            RecordKind::Sequence | RecordKind::Unknown(_) => {}
        }
    }

    fn flow_name(&self, flow: u32) -> Option<String> {
        Some(self.strings.name(flow).to_owned())
    }

    /// Removes frames `from..` of a thread's stack, recording them as call spans.
    fn close_frames(&mut self, thread: i32, from: usize, micros: i64, threw: bool) {
        let Some(stack) = self.threads.get_mut(&thread) else { return };
        if from >= stack.len() {
            return;
        }
        let removed: Vec<Frame> = stack.drain(from..).collect();
        for (i, f) in removed.into_iter().enumerate() {
            let Some(flow) = f.flow else { continue };
            if self.spans.len() < MAX_SPANS {
                self.spans.push(SpanRec {
                    thread,
                    flow,
                    start: f.start,
                    end: micros.max(f.start),
                    depth: (from + i) as u32,
                    resumed: f.resumed,
                    // Only frames above the one that returned normally were unwound.
                    threw: threw || i > 0,
                    calling: f.calling,
                });
            }
        }
    }

    fn pop(&mut self, thread: i32, method: i32, micros: i64) {
        let Some(pos) = self.threads.get(&thread).and_then(|s| s.iter().rposition(|f| f.method == method)) else { return };
        let above_threw = self.threads.get(&thread).is_some_and(|s| pos + 1 < s.len());
        if above_threw {
            self.close_frames(thread, pos + 1, micros, true);
        }
        self.close_frames(thread, pos, micros, false);
        let Some(stack) = self.threads.get_mut(&thread) else { return };
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
        for (flow, node) in self.changed_throws.drain() {
            let count = self.throws[&(flow, node)];
            frame.throws.entry(self.strings.name(flow).to_owned()).or_default().insert(self.strings.name(node).to_owned(), count);
        }
        frame
    }

    /// True when `take_frame` would report anything new.
    pub fn has_changes(&self) -> bool {
        !(self.changed_counts.is_empty()
            && self.changed_calls.is_empty()
            && self.changed_values.is_empty()
            && self.changed_vars.is_empty()
            && self.changed_throws.is_empty())
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
        for (&(flow, node), &count) in &self.throws {
            frame.throws.entry(self.strings.name(flow).to_owned()).or_default().insert(self.strings.name(node).to_owned(), count);
        }
        frame
    }

    /// Function executions over time, for the flame chart (finished ones plus those still
    /// running), ordered by start time. Times are microseconds since the program started tracing.
    pub fn calls(&self, limit: usize) -> CallsPage {
        let mut spans = self.all_spans();
        let total = spans.len() as u32;
        spans.truncate(limit);
        CallsPage { total, start: self.first_micros.unwrap_or(0), end: self.last_micros, spans }
    }

    /// The execution `root` and every call made under it (its callees, theirs, ... and the
    /// continuations of resumed async methods it started), in start order, at most `limit`.
    /// Empty when `root` is unknown.
    pub fn span_tree(&self, root: u32, limit: usize) -> CallsPage {
        let all = self.all_spans();
        let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut root_at = None;
        for (i, s) in all.iter().enumerate() {
            if s.id == root {
                root_at = Some(i);
            }
            // A continuation started by no traced caller belongs to the invocation it resumes.
            if let Some(p) = s.parent.or(s.resumes) {
                children.entry(p).or_default().push(i);
            }
        }
        let Some(root_at) = root_at else { return CallsPage::default() };
        let mut picked = vec![root_at];
        let mut truncated = false;
        let mut queue = std::collections::VecDeque::from([root]);
        while let Some(id) = queue.pop_front() {
            for &i in children.get(&id).map(Vec::as_slice).unwrap_or(&[]) {
                if picked.len() >= limit.max(1) {
                    truncated = true;
                    break;
                }
                picked.push(i);
                queue.push_back(all[i].id);
            }
        }
        // `total` counts the whole tree only when it fit.
        let total = if truncated { picked.len() as u32 + 1 } else { picked.len() as u32 };
        picked.sort_by_key(|&i| (all[i].start, all[i].depth, all[i].id));
        let spans: Vec<CallSpan> = picked.into_iter().map(|i| all[i].clone()).collect();
        let start = spans.first().map(|s| s.start).unwrap_or(0);
        let end = spans.iter().map(|s| s.end).max().unwrap_or(start);
        CallsPage { total, start, end, spans }
    }

    fn call_span(&self, thread: i32, flow: u32, depth: u32, times: (i64, i64), state: (bool, bool, bool), calling: &Calling) -> CallSpan {
        let (start, end) = times;
        let (resumed, threw, open) = state;
        CallSpan {
            thread,
            flow: self.strings.name(flow).to_owned(),
            start,
            end,
            depth,
            resumed,
            threw,
            open,
            id: calling.id,
            parent: calling.parent,
            call_site: calling.call_site.map(|n| self.strings.name(n).to_owned()),
            resumes: calling.resumes,
            args: calling.args.clone(),
            ret: calling.ret.clone(),
            exception: calling.exception.clone(),
        }
    }

    /// Finished executions plus those still running, by start time.
    fn all_spans(&self) -> Vec<CallSpan> {
        let mut spans: Vec<CallSpan> = self
            .spans
            .iter()
            .map(|s| self.call_span(s.thread, s.flow, s.depth, (s.start, s.end), (s.resumed, s.threw, false), &s.calling))
            .collect();
        for (&thread, stack) in &self.threads {
            for (depth, f) in stack.iter().enumerate() {
                if let Some(flow) = f.flow {
                    let end = self.last_micros.max(f.start);
                    spans.push(self.call_span(thread, flow, depth as u32, (f.start, end), (f.resumed, false, true), &f.calling));
                }
            }
        }
        spans.sort_by_key(|s| (s.start, s.depth, s.id));
        spans
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
    fn tail_replay_skips_frames_already_received() {
        let mut r = setup();
        let seq = |pid: i32, n: i32| Record { kind: RecordKind::Sequence, thread: pid, micros: 0, method: 0, arg: n, text: None };
        // Frame 1 arrived over the socket.
        r.record(&seq(42, 1));
        r.record(&rec(RecordKind::Enter, 0, None));
        r.record(&rec(RecordKind::Step, 1, None));
        assert_eq!(r.last_sequence(42), 1);
        // The tail holds frame 1 again (its write was not confirmed) and frame 2.
        let tail = vec![seq(42, 1), rec(RecordKind::Step, 1, None), seq(42, 2), rec(RecordKind::Step, 1, None), rec(RecordKind::Step, 2, None)];
        assert_eq!(r.replay_tail(&tail), 3, "frame 2 only (its sequence record and two steps)");
        let frame = r.take_frame();
        assert_eq!(frame.counts["S.M(int)"]["a"], 2, "frame 1 is not counted twice");
        assert_eq!(frame.counts["S.M(int)"]["b"], 1);
        assert_eq!(frame.events, 4, "sequence records are not events");
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

        let calls = r.calls(100);
        assert_eq!(calls.total, 2);
        assert!(calls.spans.iter().all(|s| s.flow == "S.M(int)" && !s.threw && !s.open && s.depth == 0));
    }

    fn at(kind: RecordKind, method: i32, arg: i32, micros: i64, text: Option<&str>) -> Record {
        Record { kind, thread: 1, micros, method, arg, text: text.map(str::to_owned) }
    }

    /// A callee throws; the caller catches: the callee frame closes as `threw`, the throw is
    /// attached to the statement that was running.
    #[test]
    fn exceptions_mark_the_statement_and_unwind_frames() {
        let mut r = setup();
        r.record(&at(RecordKind::Enter, 1, 0, 0, None)); // outer M
        r.record(&at(RecordKind::Step, 1, 1, 1, None)); // at `a`
        r.record(&at(RecordKind::Enter, 1, 0, 2, None)); // recursive M
        r.record(&at(RecordKind::Step, 1, 2, 3, None)); // at `b`
        r.record(&at(RecordKind::Throw, 0, 0, 4, Some("InvalidOperationException: nope")));
        // Execution continues in the outer frame (a catch), so the inner one was unwound.
        // Both frames share the method id, so emulate the outer frame stepping by popping once.
        r.record(&at(RecordKind::Exit, 1, 0, 5, None));
        let frame = r.take_frame();
        assert_eq!(frame.throws["S.M(int)"]["b"], 1);
        assert_eq!(frame.values["S.M(int)"]["b"], "⚠ InvalidOperationException: nope");
        let calls = r.calls(10);
        assert_eq!(calls.spans.len(), 2, "one closed, one still open");
        assert!(calls.spans.iter().any(|s| s.open && s.depth == 0));
        assert!(calls.spans.iter().any(|s| !s.open && s.depth == 1 && s.end == 5));
    }

    fn on(kind: RecordKind, thread: i32, method: i32, arg: i32, micros: i64, text: Option<&str>) -> Record {
        Record { kind, thread, micros, method, arg, text: text.map(str::to_owned) }
    }

    /// `setup()` plus method 2: the resume half of the same function (an async state machine).
    fn setup_async() -> Recorder {
        let flow = FlowGraph {
            symbol: "S.M(int)".into(),
            name: "M".into(),
            language: Language::Csharp,
            span: span(0, 100),
            nodes: vec![node("entry", Concept::Entry, 0, 10), node("a", Concept::Var, 20, 30), node("b", Concept::Return, 40, 50)],
            edges: Vec::new(),
        };
        let method = |id: i32, kind: &str| MethodMap {
            id,
            name: "S::M".into(),
            kind: kind.into(),
            points: vec![PointMap { file: "A.cs".into(), start: 12, end: 13 }, PointMap { file: "A.cs".into(), start: 40, end: 50 }],
            locals: HashMap::new(),
            params: Vec::new(),
        };
        let map = TraceMap { version: 1, methods: vec![method(1, "method"), method(2, "resume")] };
        Recorder::new(&map, Resolver::new([&flow]))
    }

    #[test]
    fn spans_know_their_caller_call_site_and_values() {
        let mut r = setup();
        r.record(&at(RecordKind::Enter, 1, 0, 0, None)); // outer call, id 0
        r.record(&at(RecordKind::Arg, 1, 0, 0, Some("5")));
        r.record(&at(RecordKind::Step, 1, 1, 1, None)); // at `a`: the call statement
        r.record(&at(RecordKind::Enter, 1, 0, 2, None)); // recursive call, id 1
        r.record(&at(RecordKind::Arg, 1, 0, 2, Some("4")));
        r.record(&at(RecordKind::Step, 1, 2, 3, None));
        r.record(&at(RecordKind::Return, 1, 0, 4, Some("1")));
        r.record(&at(RecordKind::Step, 1, 2, 5, None));
        r.record(&at(RecordKind::Return, 1, 0, 6, Some("2")));
        let page = r.calls(10);
        assert_eq!(page.spans.len(), 2);
        let outer = &page.spans[0];
        let inner = &page.spans[1];
        assert_eq!((outer.id, outer.parent, outer.call_site.as_deref()), (0, None, None));
        assert_eq!((inner.id, inner.parent, inner.call_site.as_deref()), (1, Some(0), Some("a")));
        assert_eq!(outer.args, vec!["n = 5"]);
        assert_eq!(inner.args, vec!["n = 4"]);
        assert_eq!(outer.ret.as_deref(), Some("2"));
        assert_eq!(inner.ret.as_deref(), Some("1"));
        assert!(outer.exception.is_none() && outer.resumes.is_none());
    }

    #[test]
    fn threads_keep_separate_callers() {
        let mut r = setup();
        r.record(&on(RecordKind::Enter, 1, 1, 0, 0, None)); // id 0 on thread 1
        r.record(&on(RecordKind::Enter, 2, 1, 0, 1, None)); // id 1 on thread 2: no caller there
        r.record(&on(RecordKind::Step, 1, 1, 1, 2, None));
        r.record(&on(RecordKind::Enter, 1, 1, 0, 3, None)); // id 2, called by id 0
        r.record(&on(RecordKind::Exit, 1, 1, 0, 4, None));
        r.record(&on(RecordKind::Exit, 2, 1, 0, 5, None));
        r.record(&on(RecordKind::Exit, 1, 1, 0, 6, None));
        let by_id = |id: u32| r.calls(10).spans.into_iter().find(|s| s.id == id).expect("span");
        assert_eq!(by_id(1).parent, None, "another thread's frame is not a caller");
        assert_eq!(by_id(2).parent, Some(0));
        assert_eq!(by_id(2).call_site.as_deref(), Some("a"));
        let tree = r.span_tree(0, 100);
        assert_eq!(tree.spans.iter().map(|s| s.id).collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!(tree.total, 2);
        assert!(r.span_tree(99, 100).spans.is_empty(), "unknown root");
    }

    #[test]
    fn an_exception_is_kept_on_the_call_it_left() {
        let mut r = setup();
        r.record(&at(RecordKind::Enter, 1, 0, 0, None)); // id 0
        r.record(&at(RecordKind::Step, 1, 1, 1, None));
        r.record(&at(RecordKind::Enter, 1, 0, 2, None)); // id 1
        r.record(&at(RecordKind::Step, 1, 2, 3, None));
        r.record(&at(RecordKind::Throw, 0, 0, 4, Some("InvalidOperationException: nope")));
        r.record(&at(RecordKind::Exit, 1, 0, 5, None));
        let inner = r.calls(10).spans.into_iter().find(|s| s.id == 1).expect("inner");
        assert_eq!(inner.exception.as_deref(), Some("InvalidOperationException: nope"));
        assert_eq!(inner.parent, Some(0));
    }

    #[test]
    fn resumed_async_calls_point_at_the_invocation_they_continue() {
        let mut r = setup_async();
        r.record(&on(RecordKind::Enter, 1, 1, 0, 0, None)); // id 0: the call
        r.record(&on(RecordKind::Exit, 1, 1, 0, 1, None)); // returns at the first await
        r.record(&on(RecordKind::Enter, 7, 2, 0, 5, None)); // id 1: continuation on a pool thread
        r.record(&on(RecordKind::Exit, 7, 2, 0, 6, None));
        let resumed = r.calls(10).spans.into_iter().find(|s| s.id == 1).expect("continuation");
        assert!(resumed.resumed);
        assert_eq!(resumed.parent, None);
        assert_eq!(resumed.resumes, Some(0));
        let tree = r.span_tree(0, 100);
        assert_eq!(tree.spans.iter().map(|s| s.id).collect::<Vec<_>>(), vec![0, 1], "the continuation belongs to the call");
    }

    #[test]
    fn long_values_are_shortened_and_trees_are_capped() {
        let mut r = setup();
        let long = "x".repeat(500);
        r.record(&at(RecordKind::Enter, 1, 0, 0, None));
        r.record(&at(RecordKind::Arg, 1, 0, 0, Some(&long)));
        r.record(&at(RecordKind::Step, 1, 1, 1, None));
        for i in 0..5 {
            r.record(&at(RecordKind::Enter, 1, 0, 2 + i * 2, None));
            r.record(&at(RecordKind::Exit, 1, 0, 3 + i * 2, None));
        }
        let page = r.calls(10);
        let root = page.spans.iter().find(|s| s.id == 0).expect("root");
        assert_eq!(root.args[0].chars().count(), MAX_SPAN_TEXT);
        assert!(root.args[0].ends_with('\u{2026}'));
        let tree = r.span_tree(0, 3);
        assert_eq!(tree.spans.len(), 3);
        assert!(tree.total > 3, "a capped tree says there is more");
        // Paging the flat list keeps global ids.
        let first = r.calls(2);
        assert_eq!(first.total, 6);
        assert!(first.spans.iter().all(|s| s.parent.is_none_or(|p| p == 0)));
    }
}
