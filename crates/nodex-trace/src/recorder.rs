//! Aggregates trace records into live frames and keeps a timeline for replay.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use ts_rs::TS;
use nodex_ir::{Language, types::{TypeShape, native_shape, portable_shape}};

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
const MAX_STEP_TYPES: usize = 64;
const MAX_TYPE_DESCRIPTORS: usize = 4096;
const MAX_TYPE_OBSERVATIONS: usize = 2_000_000;

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
    /// The previous mapped step in this execution, when no observed gap separates them.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub previous_node: Option<String>,
    /// Values written while this step ran (`name = value`, `→ value` for returns).
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub observations: Option<Vec<TypeObservation>>,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct TypeObservation {
    pub name: String,
    pub slot: i32,
    pub parameter: bool,
    pub shape: TypeShape,
    pub native: String,
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
    Step { micros: i64, thread: i32, flow: u32, call: Option<u32>, node: Option<u32>, previous: Option<u32> },
    Value { call: u32, text: String },
    Gap(Option<u32>),
}

struct RecordedType {
    name: u32,
    slot: i32,
    parameter: bool,
    descriptor: u32,
}

/// A resolved point: interned flow, and the interned node when the point lies in a statement.
type Resolved = Option<(u32, Option<u32>)>;

struct Frame {
    method: i32,
    flow: Option<u32>,
    last_node: Option<u32>,
    last_step: Option<(u32, u32)>,
    entered_after_drops: u64,
    args: Vec<String>,
    early_types: Vec<RecordedType>,
    /// Timeline index of the recorded step and how many type observations it holds.
    observation_step: Option<(usize, usize)>,
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
    observations: Vec<(usize, RecordedType)>,
    descriptors: Vec<(TypeShape, String)>,
    descriptor_ids: HashMap<String, u32>,
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
            observations: Vec::new(),
            descriptors: Vec::new(),
            descriptor_ids: HashMap::new(),
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
                    last_step: None,
                    entered_after_drops: self.dropped,
                    args: Vec::new(),
                    start: r.micros,
                    resumed: is_resume,
                    early_types: Vec::new(),
                    observation_step: None,
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
                let point = self.point(r.method, r.arg);
                let timeline_index = (point.is_some() && self.timeline.len() < MAX_TIMELINE).then_some(self.timeline.len());
                let observation_room = MAX_TYPE_OBSERVATIONS - self.observations.len();
                let frame = self.threads.get_mut(&r.thread).and_then(|s| s.last_mut())
                    .filter(|f| f.method == r.method && f.entered_after_drops == self.dropped);
                let (call, previous, early_types) = frame.map(|f| {
                    let previous = f.last_step;
                    f.last_step = point.and_then(|(flow, node)| node.map(|n| (flow, n)));
                    let mut early = if f.last_step.is_some() { std::mem::take(&mut f.early_types) } else { Vec::new() };
                    early.truncate(observation_room);
                    f.observation_step = timeline_index.map(|index| (index, early.len()));
                    (Some(f.calling.id), previous, early)
                }).unwrap_or_default();
                let Some((flow, node)) = point else {
                    if call.is_some() && self.timeline.len() < MAX_TIMELINE {
                        self.timeline.push(Entry::Gap(call));
                    }
                    return;
                };
                let previous = previous.filter(|(prior_flow, prior_node)| {
                    *prior_flow == flow && node.is_some_and(|n| n != *prior_node)
                }).map(|(_, prior_node)| prior_node);
                if let Some(n) = node {
                    *self.counts.entry((flow, n)).or_default() += 1;
                    self.changed_counts.insert((flow, n));
                }
                if let Some(frame) = self.threads.get_mut(&r.thread).and_then(|s| s.iter_mut().rev().find(|f| f.method == r.method))
                    && node.is_some() {
                        frame.last_node = node;
                    }
                self.active.insert(r.thread, (flow, node));
                if let Some(index) = timeline_index {
                    self.timeline.push(Entry::Step { micros: r.micros, thread: r.thread, flow, call, node, previous });
                    self.observations.extend(early_types.into_iter().map(|observation| (index, observation)));
                }
            }
            RecordKind::LocalType | RecordKind::ArgType => self.record_type(r),
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
                        (f.flow, f.last_node, f.args.join(", "), f.calling.id)
                    });
                let Some((Some(flow), last_node, args, call)) = frame_info else { return };
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
                    self.timeline.push(Entry::Value { call, text });
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
                let current = self.threads.get(&r.thread).and_then(|s| s.last()).map(|f| (f.flow, f.last_node, f.calling.id));
                let Some((Some(flow), Some(node), call)) = current else { return };
                *self.throws.entry((flow, node)).or_default() += 1;
                self.changed_throws.insert((flow, node));
                let text = format!("⚠ {}", r.text.clone().unwrap_or_else(|| "exception".into()));
                self.set_value(flow, node, text.clone());
                if self.timeline.len() < MAX_TIMELINE {
                    self.timeline.push(Entry::Value { call, text });
                }
            }
            RecordKind::Dropped => {
                self.dropped += r.arg.max(0) as u64;
                if r.arg > 0 && self.timeline.len() < MAX_TIMELINE {
                    self.timeline.push(Entry::Gap(None));
                }
            }
            RecordKind::Sequence | RecordKind::Unknown(_) => {}
        }
    }

    fn flow_name(&self, flow: u32) -> Option<String> {
        Some(self.strings.name(flow).to_owned())
    }

    fn record_type(&mut self, record: &Record) {
        if self.observations.len() >= MAX_TYPE_OBSERVATIONS { return; }
        let parameter = record.kind == RecordKind::ArgType;
        let Some(method) = self.methods.get(&record.method) else { return };
        let Some(name) = (if parameter { method.params.get(record.arg as usize) } else { method.locals.get(&record.arg.to_string()) }) else { return };
        let Some(frame) = self.threads.get_mut(&record.thread)
            .and_then(|stack| stack.iter_mut().rev().find(|frame| frame.method == record.method))
            .filter(|frame| frame.flow.is_some() && frame.entered_after_drops == self.dropped)
        else { return };
        let early = parameter && frame.last_node.is_none();
        if early {
            if frame.early_types.len() >= MAX_STEP_TYPES || self.timeline.len() >= MAX_TIMELINE { return; }
        } else if frame.observation_step.is_none_or(|(_, count)| count >= MAX_STEP_TYPES) {
            return;
        }
        let Some(text) = record.text.as_deref().filter(|text| text.len() <= 2048) else { return };
        let descriptor = if let Some(&id) = self.descriptor_ids.get(text) {
            id
        } else {
            if self.descriptors.len() >= MAX_TYPE_DESCRIPTORS { return; }
            let Some((portable, native)) = text.split_once('\n') else { return };
            let shape = if portable.is_empty() { native_shape(Language::Rust, native) } else { portable_shape(portable) };
            let id = self.descriptors.len() as u32;
            self.descriptor_ids.insert(text.into(), id);
            self.descriptors.push((shape, native.into()));
            id
        };
        let observation = RecordedType { name: self.strings.id(name), slot: record.arg, parameter, descriptor };
        if early {
            frame.early_types.push(observation);
        } else if let Some((step, count)) = &mut frame.observation_step {
            self.observations.push((*step, observation));
            *count += 1;
        }
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
        // Values belong to the latest step of the same execution, including recursive calls.
        let mut last: HashMap<u32, usize> = HashMap::new();
        let selected = offset..offset.saturating_add(limit);
        let mut observed_steps = HashMap::new();
        for (entry, e) in self.timeline.iter().enumerate() {
            match e {
                Entry::Step { micros, thread, flow, call, node, previous } => {
                    if *flow == flow_id {
                        if selected.contains(&steps.len()) {
                            observed_steps.insert(entry, steps.len());
                        }
                        steps.push(StepInfo {
                            index: steps.len() as u32,
                            micros: *micros,
                            thread: *thread,
                            node: node.map(|n| self.strings.name(n).to_owned()),
                            previous_node: previous.map(|n| self.strings.name(n).to_owned()),
                            values: Vec::new(),
                            observations: None,
                        });
                        if let Some(call) = call {
                            last.insert(*call, steps.len() - 1);
                        }
                    } else if let Some(call) = call {
                        last.remove(call);
                    }
                }
                Entry::Value { call, text } => {
                    if let Some(i) = last.get(call) {
                        steps[*i].values.push(text.clone());
                    }
                }
                Entry::Gap(Some(call)) => { last.remove(call); }
                Entry::Gap(None) => { last.clear(); }
            }
        }
        for (entry, observation) in &self.observations {
            let Some(&i) = observed_steps.get(entry) else { continue };
            let (shape, native) = &self.descriptors[observation.descriptor as usize];
            steps[i].observations.get_or_insert_with(Vec::new).push(TypeObservation {
                name: self.strings.name(observation.name).into(), slot: observation.slot,
                parameter: observation.parameter, shape: shape.clone(), native: native.clone(),
            });
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
            syntax_sites: None,
            id: id.into(),
            concept,
            label: id.into(),
            focus: None,
            code: id.into(),
            span: span(start, end),
            flavors: Vec::new(),
            purity: Purity::Impure,
            inputs: Vec::new(),
            outputs: Vec::new(),
            expr: None,
            writes: Vec::new(),
            binding_writes: Vec::new(),
            calls: Vec::new(),
        }
    }

    fn setup() -> Recorder {
        let flow = FlowGraph {
            symbol: "S.M(int)".into(),
            name: "M".into(),
            language: Language::Csharp,
            span: span(0, 100),
            source_revision: None,
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
    fn replay_keeps_predecessors_and_values_within_each_invocation() {
        let mut r = setup();
        let events = [
            (RecordKind::Enter, 0, None),
            (RecordKind::Step, 1, None),
            (RecordKind::Enter, 0, None),
            (RecordKind::Arg, 0, Some("inner")),
            (RecordKind::Step, 2, None),
            (RecordKind::Return, 0, Some("inner return")),
            (RecordKind::Local, 0, Some("outer local")),
            (RecordKind::Step, 2, None),
            (RecordKind::Step, 0, None),
            (RecordKind::Step, 1, None),
            (RecordKind::Dropped, 1, None),
            (RecordKind::Step, 2, None),
            (RecordKind::Step, 1, None),
        ];
        for (kind, arg, text) in events {
            r.record(&rec(kind, arg, text));
        }
        let page = r.steps("S.M(int)", 0, 20);
        assert_eq!(page.steps.iter().map(|s| s.previous_node.as_deref()).collect::<Vec<_>>(),
            vec![None, None, Some("a"), None, None, None, None]);
        assert_eq!(page.steps[0].values, ["x = outer local"]);
        assert_eq!(page.steps[1].values, ["→ inner return"]);
        assert_eq!(r.steps("S.M(int)", 2, 1).steps[0].previous_node.as_deref(), Some("a"));
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
        assert!(page.steps.iter().all(|step| step.observations.is_none()), "legacy traces keep their values");

        let calls = r.calls(100);
        assert_eq!(calls.total, 2);
        assert!(calls.spans.iter().all(|s| s.flow == "S.M(int)" && !s.threw && !s.open && s.depth == 0));
    }

    #[test]
    fn observations_follow_the_capture_step_and_recursive_invocation() {
        use RecordKind::*;
        let mut recorder = setup();
        for (kind, slot, text) in [
            (Enter, 0, None),
            (ArgType, 0, Some("Int32\nSystem.Int32")),
            (Step, 0, None), // no mapped node yet
            (Step, 1, None),
            (Local, 0, Some("original preview")),
            (LocalType, 0, Some("String\nstring")),
            (Enter, 0, None),
            (ArgType, 0, Some("Bool\nboolean")),
            (Step, 1, None),
            (LocalType, 0, Some("Array<?>\nArray")),
            (Exit, 0, None),
            (Step, 2, None),
            (LocalType, 0, Some("Number\nnumber")),
        ] {
            recorder.record(&rec(kind, slot, text));
        }
        let page = recorder.steps("S.M(int)", 0, 10);
        assert_eq!(recorder.timeline.len(), 5, "optional observations use no replay entries");
        assert!(page.steps[0].observations.is_none());
        assert_eq!(page.steps[1].values, ["x = original preview"]);
        let observations = |i: usize| page.steps[i].observations.as_deref().unwrap();
        assert_eq!(observations(1).iter().map(|o| (&*o.name, o.parameter, &*o.native)).collect::<Vec<_>>(),
            [("n", true, "System.Int32"), ("x", false, "string")]);
        assert_eq!(observations(2)[0].shape, TypeShape::scalar("Bool"));
        assert_eq!(observations(2)[1].shape, portable_shape("Array<?>"));
        assert_eq!(observations(3)[0].shape, TypeShape::scalar("Number"));
        assert_eq!(recorder.steps("S.M(int)", 2, 1).steps[0].observations.as_deref(), Some(observations(2)));
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
            source_revision: None,
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
