//! On-demand resolution of a Rust borrow through lexical bindings and exact calls.

use std::collections::HashMap;

use nodex_ir::{
    BindingOrigin, Concept, FlowGraph, FlowNode, Pin, Span, SymbolId, ValueKind,
    borrow::{BorrowArgument, BorrowBinding, BorrowCall, BorrowFact, BorrowInput, BorrowModel, BorrowPath, BorrowPreview, BorrowRoot, BorrowRule, BorrowSource},
    semantic::{ByteRange, RustNominalType},
    types::TypeShape,
};
use serde::Serialize;
use ts_rs::TS;

use crate::{knowledge::RawCallSite, project::Project};
pub use nodex_ir::borrow::BorrowOriginKind;

const PAGE_SIZE: usize = 20;
const MAX_DEPTH: usize = 8;
const MAX_RESULTS: usize = 240;

#[derive(Default)]
pub(crate) struct CallIndex {
    outgoing: HashMap<(String, u32, u32), Option<SymbolId>>,
    incoming: HashMap<SymbolId, Vec<CallLink>>,
    method_owners: HashMap<SymbolId, SymbolId>,
    nominal_types: HashMap<ByteRange, Option<(SymbolId, bool)>>,
    rules: HashMap<ByteRange, Option<BorrowRule>>,
}

struct CallLink {
    caller: SymbolId,
    file: String,
    start: u32,
    end: u32,
}

impl CallIndex {
    pub(crate) fn new<'a>(
        calls: &[RawCallSite],
        graph: &nodex_ir::StructureGraph,
        locator: &crate::semantic::Locator,
        types: impl Iterator<Item = &'a RustNominalType>,
        borrow_facts: impl Iterator<Item = &'a BorrowFact>,
    ) -> Self {
        let mut index = Self::default();
        for fact in borrow_facts {
            index.rules.entry(fact.site.clone())
                .and_modify(|rule| {
                    if rule.as_ref() != Some(&fact.rule) {
                        *rule = None;
                    }
                })
                .or_insert_with(|| Some(fact.rule.clone()));
        }
        let rust: std::collections::HashSet<_> = graph
            .symbols
            .iter()
            .filter(|s| s.language == nodex_ir::Language::Rust && s.has_flow)
            .map(|s| s.id.as_str())
            .collect();
        index
            .method_owners
            .extend(graph.symbols.iter().filter_map(|symbol| {
                let parent = symbol.parent.as_ref()?;
                (symbol.kind == nodex_ir::SymbolKind::Method && rust.contains(symbol.id.as_str()))
                    .then(|| (symbol.id.clone(), parent.clone()))
            }));
        for fact in types {
            if let Some(owner) = locator.type_at(&fact.declaration) {
                let evidence = (owner.to_owned(), fact.by_reference);
                index
                    .nominal_types
                    .entry(fact.site.clone())
                    .and_modify(|previous| {
                        if previous.as_ref() != Some(&evidence) {
                            *previous = None;
                        }
                    })
                    .or_insert(Some(evidence));
            }
        }
        let traits: std::collections::HashSet<_> = graph
            .symbols
            .iter()
            .filter(|s| s.kind == nodex_ir::SymbolKind::Trait)
            .map(|s| s.id.as_str())
            .collect();
        let dispatch: std::collections::HashSet<_> = graph
            .symbols
            .iter()
            .filter(|s| {
                s.parent
                    .as_deref()
                    .is_some_and(|parent| traits.contains(parent))
            })
            .map(|s| s.id.as_str())
            .collect();
        for call in calls
            .iter()
            .filter(|call| rust.contains(call.owner.as_str()))
        {
            index
                .outgoing
                .entry((call.file.clone(), call.start, call.end))
                .and_modify(|target| {
                    if target.as_deref() != Some(&call.target) {
                        *target = None;
                    }
                })
                .or_insert_with(|| Some(call.target.clone()));
        }
        index.outgoing.retain(|_, target| {
            target
                .as_deref()
                .is_some_and(|id| rust.contains(id) && !dispatch.contains(id))
        });
        for call in calls
            .iter()
            .filter(|call| rust.contains(call.owner.as_str()))
        {
            let key = (call.file.clone(), call.start, call.end);
            if index.outgoing.get(&key).and_then(Option::as_deref) == Some(&call.target) {
                index
                    .incoming
                    .entry(call.target.clone())
                    .or_default()
                    .push(CallLink {
                        caller: call.owner.clone(),
                        file: call.file.clone(),
                        start: call.start,
                        end: call.end,
                    });
            }
        }
        for sites in index.incoming.values_mut() {
            sites.sort_by(|a, b| {
                (&a.file, a.start, a.end, &a.caller).cmp(&(&b.file, b.start, b.end, &b.caller))
            });
            sites.dedup_by(|a, b| {
                (&a.file, a.start, a.end, &a.caller) == (&b.file, b.start, b.end, &b.caller)
            });
        }
        index
    }

    fn target(&self, span: &Span) -> Option<&str> {
        self.outgoing
            .get(&(span.file.clone(), span.start_byte, span.end_byte))?
            .as_deref()
    }

    fn rule(&self, span: &Span) -> Option<&BorrowRule> {
        self.rules.get(&ByteRange {
            file: span.file.clone(), start: span.start_byte, end: span.end_byte,
        })?.as_ref()
    }

    fn actual_paths<'a>(
        &self,
        callee: &FlowGraph,
        argument: &'a BorrowArgument,
        receiver: bool,
    ) -> &'a [BorrowPath] {
        if !receiver {
            return &argument.value;
        }
        let Some(owner) = self.method_owners.get(&callee.symbol) else {
            return &[];
        };
        let [place] = argument.place.as_slice() else {
            return &[];
        };
        if !place.projection.is_empty() {
            return &[];
        }
        let span = match &place.root {
            BorrowRoot::Local(binding) => &binding.span,
            BorrowRoot::Temporary(span) => span,
            _ => return &[],
        };
        let site = ByteRange {
            file: span.file.clone(),
            start: span.start_byte,
            end: span.end_byte,
        };
        match self.nominal_types.get(&site).and_then(Option::as_ref) {
            Some((ty, false)) if ty == owner => &argument.place,
            Some((ty, true)) if ty == owner => &argument.value,
            // The call target alone cannot prove that a user-defined Deref
            // accesses storage inside the receiver.
            _ => &[],
        }
    }
}

#[derive(Serialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct BorrowLocation {
    pub label: String,
    pub flow: SymbolId,
    pub node: Option<String>,
    pub pin: Option<String>,
    pub span: Span,
    pub revision: Option<String>,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct BorrowOrigin {
    pub kind: BorrowOriginKind,
    pub owner: Option<BorrowLocation>,
    pub borrowed: Option<String>,
    pub through: Vec<BorrowLocation>,
    pub call: Option<BorrowLocation>,
}

impl BorrowOrigin {
    fn empty(kind: BorrowOriginKind) -> Self {
        Self {
            kind,
            owner: None,
            borrowed: None,
            through: Vec::new(),
            call: None,
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct BorrowOrigins {
    pub origins: Vec<BorrowOrigin>,
    pub next_offset: Option<usize>,
    pub partial: bool,
}

pub(crate) fn resolve(
    project: &Project,
    calls: &CallIndex,
    symbol: &str,
    node_id: &str,
    pin: Option<&str>,
    expr_path: Option<&str>,
    offset: usize,
) -> Option<BorrowOrigins> {
    let (flow, model) = project.borrow_model(symbol)?;
    let node = flow.node(node_id)?;
    let paths = if let Some(path) = expr_path {
        let mut parts = path.split('.');
        if parts.next()? != "0" {
            return None;
        }
        let mut expr = node.expr.as_ref()?;
        for index in parts {
            expr = expr.children.get(index.parse::<usize>().ok()?)?;
        }
        let anchor = expr
            .syntax_sites
            .iter()
            .flatten()
            .filter(|site| matches!(site.key.as_str(), "borrow" | "borrow-mut"))
            .min_by_key(|site| site.span.start_byte)
            .map(|site| &site.span);
        paths_at(model, anchor.map_or(expr.span.start_byte, |s| s.start_byte), expr.span.end_byte).to_vec()
    } else if let Some(id) = pin {
        let selected = node
            .inputs
            .iter()
            .chain(&node.outputs)
            .find(|p| p.id == id)?;
        pin_paths(model, node, selected)
    } else {
        Vec::new()
    };
    let mut query = Resolver {
        project,
        calls,
        offset,
        page_end: None,
        next_offset: None,
        partial: false,
        visits: 0,
        follow_callers: true,
    };
    let mut origins = query.paths(flow, model, &paths, &[], 0, None);
    if origins.is_empty() {
        origins.push(BorrowOrigin::empty(BorrowOriginKind::Unknown));
    }
    Some(BorrowOrigins {
        origins,
        next_offset: query.next_offset,
        partial: query.partial,
    })
}

fn paths_at(model: &BorrowModel, start: u32, end: u32) -> &[BorrowPath] {
    let at = model.uses.partition_point(|site| site.span.start_byte < start);
    model.uses[at..].iter().take_while(|site| site.span.start_byte == start)
        .find(|site| site.span.end_byte >= end).map_or(&[], |site| site.paths.as_slice())
}

fn pin_paths(model: &BorrowModel, node: &FlowNode, pin: &Pin) -> Vec<BorrowPath> {
    if let Some(decl) = &pin.declaration {
        return paths_at(model, decl.start_byte, decl.end_byte).to_vec();
    }
    let Some(name) = pin.id.strip_prefix("var:").or_else(|| pin.id.strip_prefix("use:"))
        .or_else(|| pin.id.strip_prefix("write:")) else { return Vec::new() };
    let at = model.uses.partition_point(|site| site.span.start_byte < node.span.start_byte);
    let mut paths = Vec::new();
    for site in model.uses[at..].iter().take_while(|site| node.span.contains_byte(site.span.start_byte)) {
        for path in &site.paths {
            if path.through.last().is_some_and(|binding| binding.name == name) && !paths.contains(path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

pub(crate) fn attach_previews(project: &Project, calls: &CallIndex, flow: &mut FlowGraph) {
    let Some((_, model)) = project.borrow_model(&flow.symbol) else { return };
    let mut previews = Vec::new();
    // Alias initializers can inherit a representation from the exact read on the right.
    let mut filled_alias = false;
    for node in &mut flow.nodes {
        if node.concept == Concept::Var
            && let Some(info) = node.expr.as_ref().filter(|expr| expr.concept == Concept::Var).and_then(|expr| expr.value_info)
            && let Some(pin) = node.outputs.iter_mut().find(|pin| pin.id.starts_with("var:"))
            && pin.ty.is_none() && pin.value_info.is_none()
        {
            pin.value_info = Some(info);
            filled_alias = true;
        }
    }
    if filled_alias {
        nodex_lang_api::value_shapes::carry_to_uses(flow);
    }
    for node in &flow.nodes {
        for pin in node.inputs.iter().chain(&node.outputs) {
            let shape = pin.type_info.as_ref().map(|info| &info.shape);
            if !pin.value_info.is_some_and(|info| info.kind == ValueKind::Reference)
                && !matches!(shape, Some(TypeShape::Indirect { pointer: false, .. }))
            {
                continue;
            }
            let paths = pin_paths(model, node, pin);
            let sources = preview_sources(project, calls, flow, model, &paths);
            previews.push(BorrowPreview { node: node.id.clone(), pin: pin.id.clone(), sources });
        }
    }
    if !previews.is_empty() {
        flow.borrows = Some(previews);
    }
    if let Some(reading) = &flow.reading {
        use nodex_ir::reading::ReadingContent;
        let preview = |span: &Span| preview_sources(project, calls, flow, model,
            paths_at(model, span.start_byte, span.end_byte));
        let mut reading = reading.clone();
        for site in &mut reading {
            match &mut site.content {
                ReadingContent::Detail { borrow: true, .. } => site.borrow_sources = Some(preview(&site.span)),
                ReadingContent::Pattern { parts, .. } => {
                    for part in parts.iter_mut().filter(|part| matches!(part.access, Some(nodex_ir::Access::Borrow | nodex_ir::Access::BorrowMut))) {
                        part.borrow_sources = Some(preview(&part.span));
                    }
                }
                _ => {}
            }
        }
        flow.reading = Some(reading);
    }
}

fn preview_sources(project: &Project, calls: &CallIndex, flow: &FlowGraph, model: &BorrowModel, paths: &[BorrowPath]) -> Vec<BorrowSource> {
    let mut resolver = Resolver {
        project, calls, offset: 0, page_end: None, next_offset: None,
        partial: false, visits: 0, follow_callers: false,
    };
    let mut sources = Vec::new();
    for origin in resolver.paths(flow, model, paths, &[], 0, None) {
        let binding = origin.owner.as_ref().filter(|owner| owner.flow == flow.symbol).and_then(|owner| {
            Some(BindingOrigin { node: owner.node.clone()?, pin: owner.pin.clone()? })
        });
        let source = BorrowSource {
            kind: origin.kind, owner: origin.owner.map(|owner| owner.label), place: origin.borrowed, binding,
        };
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    if resolver.partial || sources.is_empty() {
        let unknown = BorrowSource { kind: BorrowOriginKind::Unknown, owner: None, place: None, binding: None };
        if !sources.contains(&unknown) {
            sources.push(unknown);
        }
    }
    sources
}
struct Frame<'a> {
    callee: &'a str,
    caller: &'a FlowGraph,
    caller_model: &'a BorrowModel,
    call: &'a BorrowCall,
}

struct Resolver<'a> {
    project: &'a Project,
    calls: &'a CallIndex,
    offset: usize,
    page_end: Option<usize>,
    next_offset: Option<usize>,
    partial: bool,
    visits: usize,
    follow_callers: bool,
}

impl Resolver<'_> {
    fn rule(&self, span: &Span) -> Option<&BorrowRule> {
        self.project.rust_semantics_ready().then(|| self.calls.rule(span)).flatten()
    }

    fn paths(
        &mut self,
        flow: &FlowGraph,
        model: &BorrowModel,
        paths: &[BorrowPath],
        stack: &[Frame<'_>],
        depth: usize,
        context: Option<&BorrowLocation>,
    ) -> Vec<BorrowOrigin> {
        if depth >= MAX_DEPTH || self.visits >= MAX_RESULTS {
            self.partial = true;
            return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
        }
        let mut origins = Vec::new();
        for path in paths {
            if self.visits >= MAX_RESULTS {
                self.partial = true;
                break;
            }
            self.visits += 1;
            let mut found = match &path.root {
                BorrowRoot::Local(binding) => {
                    vec![binding_origin(flow, binding, BorrowOriginKind::Local)]
                }
                BorrowRoot::Shared(binding) => {
                    vec![binding_origin(flow, binding, BorrowOriginKind::Shared)]
                }
                BorrowRoot::Static(span) => vec![span_origin(
                    flow,
                    span,
                    BorrowOriginKind::Static,
                    "Static storage",
                )],
                BorrowRoot::Temporary(span) => vec![span_origin(
                    flow,
                    span,
                    BorrowOriginKind::Temporary,
                    "Temporary value",
                )],
                BorrowRoot::Call(span) => self.call(flow, model, span, stack, depth, context),
                BorrowRoot::CallComponent { call, index } => {
                    if let Some(BorrowRule::Split { input, at }) = self.rule(call).cloned()
                        && let Some(site) = model.calls.iter().find(|site| &site.span == call)
                    {
                        let range = if *index == 0 { format!("[..{at}]") } else { format!("[{at}..]") };
                        let mut found = self.paths(flow, model, input_paths(site, input), stack, depth + 1, context);
                        for origin in &mut found {
                            if let Some(place) = &mut origin.borrowed {
                                place.push_str(&range);
                            }
                        }
                        found
                    } else {
                        vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)]
                    }
                }
                BorrowRoot::Parameter(index) => {
                    let mut origins = self.parameter(flow, *index, stack, depth, context);
                    for origin in origins.iter_mut().filter(|origin| origin.kind == BorrowOriginKind::Caller && origin.borrowed.is_none()) {
                        origin.borrowed = model.parameters.get(*index).and_then(Option::as_ref).map(|binding| binding.name.clone());
                    }
                    origins
                }
                BorrowRoot::CallbackInput { call, argument } => {
                    let paths = match self.rule(call) {
                        Some(BorrowRule::Callback { input, argument: callback }) if callback == argument => {
                            model.calls.iter().find(|site| &site.span == call)
                                .map_or(&[][..], |site| input_paths(site, *input))
                        }
                        _ => &[],
                    };
                    self.paths(flow, model, paths, stack, depth + 1, context)
                }
                BorrowRoot::Field { site, base, referent } => match self.rule(site) {
                    Some(BorrowRule::Field { by_reference: false }) =>
                        vec![binding_origin(flow, base, BorrowOriginKind::Local)],
                    Some(BorrowRule::Field { by_reference: true }) =>
                        self.paths(flow, model, referent, stack, depth + 1, context),
                    _ => vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)],
                },
                BorrowRoot::TypedPlace { binding, fallback } => {
                    use nodex_ir::{ValueKind, ValueQualifier};
                    let info = self.project.rust_semantics_ready()
                        .then(|| self.project.borrow_binding_type(&binding.span)).flatten();
                    match info.map(|info| (info.kind, info.qualifier)) {
                        Some((ValueKind::Pointer, Some(ValueQualifier::Unique))) => {
                            vec![binding_origin(flow, binding, BorrowOriginKind::Local)]
                        }
                        Some((ValueKind::Pointer, Some(ValueQualifier::Shared))) => {
                            vec![binding_origin(flow, binding, BorrowOriginKind::Shared)]
                        }
                        Some((ValueKind::Pointer | ValueKind::Value, _)) => {
                            vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)]
                        }
                        _ if !fallback.is_empty() => {
                            self.paths(flow, model, fallback, stack, depth + 1, context)
                        }
                        _ => vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)],
                    }
                }
                BorrowRoot::RawPointer | BorrowRoot::Unknown => {
                    vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)]
                }
            };
            for origin in &mut found {
                if let Some(text) = origin.borrowed.as_mut() {
                    text.push_str(&path.projection);
                }
                if origin.call.is_none() {
                    origin.call = context.cloned();
                }
                for binding in &path.through {
                    let location = binding_location(flow, binding);
                    if !origin.through.contains(&location) {
                        origin.through.push(location);
                    }
                }
            }
            for origin in found {
                if !origins.contains(&origin) {
                    origins.push(origin);
                }
            }
        }
        origins
    }

    fn call(
        &mut self,
        flow: &FlowGraph,
        model: &BorrowModel,
        span: &Span,
        stack: &[Frame<'_>],
        depth: usize,
        context: Option<&BorrowLocation>,
    ) -> Vec<BorrowOrigin> {
        let Some(call) = model.calls.iter().find(|call| &call.span == span) else {
            return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
        };
        if self.project.rust_semantics_ready() && let Some(rule) = self.calls.rule(span) {
            match rule {
                BorrowRule::Lookup { input, projection } => {
                    let mut origins = self.paths(flow, model, input_paths(call, *input), stack, depth + 1, context);
                    for origin in &mut origins {
                        if let Some(borrowed) = &mut origin.borrowed {
                            borrowed.push_str(projection);
                        }
                    }
                    return origins;
                }
                BorrowRule::Callback { argument, .. } => {
                    let paths = call.callbacks.iter().find(|callback| callback.argument == *argument)
                        .map_or(&[][..], |callback| callback.returns.as_slice());
                    return self.paths(flow, model, paths, stack, depth + 1, context);
                }
                BorrowRule::Wrap => {
                    let paths = call.arguments.first().map_or(&[][..], |arg| arg.value.as_slice());
                    return self.paths(flow, model, paths, stack, depth + 1, context);
                }
                BorrowRule::Payload { input } =>
                    return self.paths(flow, model, input_paths(call, *input), stack, depth + 1, context),
                BorrowRule::Field { .. } | BorrowRule::Split { .. } => {}
            }
        }
        let Some(target) = self.calls.target(span) else {
            return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
        };
        if target == flow.symbol || stack.iter().any(|frame| frame.callee == target) {
            self.partial = true;
            return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
        }
        let Some((callee, callee_model)) = self.project.borrow_model(target) else {
            return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
        };
        let mut frames: Vec<_> = stack.iter().map(|frame| Frame { ..*frame }).collect();
        frames.push(Frame {
            caller: flow,
            callee: target,
            caller_model: model,
            call,
        });
        self.paths(
            callee,
            callee_model,
            &callee_model.returns,
            &frames,
            depth + 1,
            context,
        )
    }

    fn parameter(
        &mut self,
        flow: &FlowGraph,
        index: usize,
        stack: &[Frame<'_>],
        depth: usize,
        context: Option<&BorrowLocation>,
    ) -> Vec<BorrowOrigin> {
        if let Some((frame, earlier)) = stack.split_last()
            && frame.callee == flow.symbol
            && let Some((argument, receiver)) = actual(frame.call, index)
        {
            let paths = self.calls.actual_paths(flow, argument, receiver);
            if paths.is_empty() {
                return vec![BorrowOrigin::empty(BorrowOriginKind::Unknown)];
            }
            return self.paths(
                frame.caller,
                frame.caller_model,
                paths,
                earlier,
                depth + 1,
                context,
            );
        }
        if !self.follow_callers {
            return vec![BorrowOrigin::empty(BorrowOriginKind::Caller)];
        }
        let Some(sites) = self.calls.incoming.get(&flow.symbol) else {
            return vec![BorrowOrigin::empty(BorrowOriginKind::Caller)];
        };
        let offset = if context.is_none() { self.offset } else { 0 };
        let top_page = context.is_none();
        let fixed_end = self.page_end.filter(|_| top_page);
        let mut end = sites
            .len()
            .min(fixed_end.unwrap_or_else(|| offset.saturating_add(PAGE_SIZE)));
        let mut page_visits = 0;
        let mut origins = Vec::new();
        for (relative, site) in sites
            .get(offset..end)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            if top_page && fixed_end.is_none() && page_visits >= MAX_RESULTS {
                end = offset + relative;
                break;
            }
            if !top_page && self.visits >= MAX_RESULTS {
                self.partial = true;
                break;
            }
            if site.caller == flow.symbol || stack.iter().any(|frame| frame.callee == site.caller) {
                self.partial = true;
                continue;
            }
            let Some((caller, caller_model)) = self.project.borrow_model(&site.caller) else {
                continue;
            };
            let Some(call) = caller_model.calls.iter().find(|call| {
                call.span.file == site.file
                    && call.span.start_byte == site.start
                    && call.span.end_byte == site.end
            }) else {
                continue;
            };
            let Some((argument, receiver)) = actual(call, index) else {
                continue;
            };
            let context = span_location(caller, &call.span, caller.name.clone());
            let mut frames: Vec<_> = stack.iter().map(|frame| Frame { ..*frame }).collect();
            // Retain the visited callee so a mutually recursive caller does not loop.
            frames.push(Frame {
                caller,
                callee: &flow.symbol,
                caller_model,
                call,
            });
            // Finish each displayed call within its own bound. Stop between calls
            // when choosing the next page so the budget never skips a caller.
            let enclosing_visits = top_page.then(|| std::mem::take(&mut self.visits));
            let mut entries = self.paths(
                caller,
                caller_model,
                self.calls.actual_paths(flow, argument, receiver),
                &frames,
                depth + 1,
                Some(&context),
            );
            if let Some(enclosing_visits) = enclosing_visits {
                page_visits += self.visits;
                self.visits = enclosing_visits;
            }
            if entries.is_empty() {
                let mut missing = BorrowOrigin::empty(BorrowOriginKind::Unknown);
                missing.call = Some(context);
                entries.push(missing);
            }
            origins.extend(entries);
        }
        if top_page {
            self.page_end = Some(end);
            self.next_offset = (end < sites.len()).then_some(end);
        } else if end < sites.len() {
            self.partial = true;
        }
        if origins.is_empty() {
            origins.push(BorrowOrigin::empty(BorrowOriginKind::Caller));
        }
        origins
    }
}

fn input_paths(call: &BorrowCall, input: BorrowInput) -> &[BorrowPath] {
    let argument = match input.argument {
        Some(index) => call.arguments.get(index),
        None => call.receiver.as_ref(),
    };
    argument.map_or(&[], |argument| if input.value { &argument.value } else { &argument.place })
}

fn actual(call: &BorrowCall, index: usize) -> Option<(&BorrowArgument, bool)> {
    if let Some(receiver) = &call.receiver {
        if index == 0 {
            return Some((receiver, true));
        }
        return call.arguments.get(index - 1).map(|arg| (arg, false));
    }
    call.arguments.get(index).map(|arg| (arg, false))
}

fn binding_origin(
    flow: &FlowGraph,
    binding: &BorrowBinding,
    kind: BorrowOriginKind,
) -> BorrowOrigin {
    BorrowOrigin {
        owner: Some(binding_location(flow, binding)),
        borrowed: Some(binding.name.clone()),
        ..BorrowOrigin::empty(kind)
    }
}

fn span_origin(flow: &FlowGraph, span: &Span, kind: BorrowOriginKind, label: &str) -> BorrowOrigin {
    BorrowOrigin {
        owner: Some(span_location(flow, span, label.into())),
        borrowed: Some(label.into()),
        ..BorrowOrigin::empty(kind)
    }
}

fn binding_location(flow: &FlowGraph, binding: &BorrowBinding) -> BorrowLocation {
    let mut location = span_location(flow, &binding.span, binding.name.clone());
    if let Some((node, pin)) = flow.nodes.iter().find_map(|n| {
        n.outputs
            .iter()
            .find(|p| p.declaration.as_ref() == Some(&binding.span))
            .map(|p| (n, p))
    }) {
        location.node = Some(node.id.clone());
        location.pin = Some(pin.id.clone());
    }
    location
}

fn span_location(flow: &FlowGraph, span: &Span, label: String) -> BorrowLocation {
    let node = flow
        .nodes
        .iter()
        .filter(|n| n.span.contains_byte(span.start_byte) && n.span.end_byte >= span.end_byte)
        .min_by_key(|n| n.span.end_byte - n.span.start_byte)
        .map(|n| n.id.clone());
    BorrowLocation {
        label,
        flow: flow.symbol.clone(),
        node,
        pin: None,
        span: span.clone(),
        revision: flow.source_revision.clone(),
    }
}
