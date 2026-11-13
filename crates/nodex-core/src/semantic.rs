//! Joins semantic facts (keyed by byte ranges) with the syntax-only IR.
//!
//! - Structure: exact `Calls`, `Instantiates`, `Inherits`/`Implements`, `Raises` and
//!   `Subscribes` edges.
//! - Flows: resolved call targets on nodes and expressions, event subscriptions and raises
//!   promoted to `Subscribe`/`Emit`, inferred types on `var` data pins.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use nodex_ir::semantic::{BindingUseFact, ByteRange, EmitFocus, ExpressionFact, ExpressionFactKind, LocalTypeFact, NavigationFact, RefKind, SemanticFacts, SemanticRef, SemanticTarget, ValueFact, ValueFactTarget};
use nodex_ir::{
    CallTarget, Concept, Expr, FlowGraph, FlowNode, Focus, Language, PinKind, StructEdge, StructEdgeKind, Symbol, SymbolId, SymbolKind,
};
use nodex_lang_api::kit::{normalize_ws, truncate};

struct Entry {
    start: u32,
    end: u32,
    id: SymbolId,
    kind: SymbolKind,
    has_flow: bool,
}

/// Finds the symbol declared at a byte offset, through declaration spans (`Symbol::origins`).
pub struct Locator {
    by_file: HashMap<String, Vec<Entry>>,
    /// Kind of every located symbol, by id.
    kinds: HashMap<SymbolId, SymbolKind>,
}

impl Locator {
    pub fn new(symbols: &[Symbol]) -> Self {
        let mut by_file: HashMap<String, Vec<Entry>> = HashMap::new();
        let mut kinds = HashMap::new();
        for s in symbols.iter().filter(|s| !s.external) {
            if !s.origins.is_empty() {
                kinds.insert(s.id.clone(), s.kind);
            }
            for o in &s.origins {
                by_file.entry(o.file.clone()).or_default().push(Entry {
                    start: o.start_byte,
                    end: o.end_byte,
                    id: s.id.clone(),
                    kind: s.kind,
                    has_flow: s.has_flow,
                });
            }
        }
        Self { by_file, kinds }
    }

    /// The smallest symbol whose declaration contains `byte` and passes `filter`.
    fn innermost(&self, file: &str, byte: u32, filter: impl Fn(&Entry) -> bool) -> Option<&Entry> {
        self.by_file
            .get(file)?
            .iter()
            .filter(|e| e.start <= byte && byte < e.end && filter(e))
            .min_by_key(|e| e.end - e.start)
    }

    /// The project symbol a target resolves to.
    pub fn target(&self, target: &SemanticTarget) -> Option<SymbolId> {
        let decl = target.decl.as_ref()?;
        let want_type = target.kind == "type";
        self.innermost(&decl.file, decl.start, |e| !want_type || e.kind.is_type())
            .map(|e| e.id.clone())
    }

    pub(crate) fn navigation_target(&self, target: &SemanticTarget) -> Option<SymbolId> {
        let decl = target.decl.as_ref()?;
        self.navigation_at(decl, &target.kind, &target.display)
    }

    pub(crate) fn navigation_at(&self, decl: &ByteRange, kind: &str, name: &str) -> Option<SymbolId> {
        self.innermost(&decl.file, decl.start, |entry| match kind {
            "type" => entry.kind.is_type(),
            "method" | "function" => matches!(entry.kind, SymbolKind::Method | SymbolKind::Function),
            "constructor" => entry.kind == SymbolKind::Constructor,
            // Destructured bindings share the declarator's range in the syntax graph.
            "variable" => entry.kind == SymbolKind::Field && entry.id.rsplit('.').next() == Some(name),
            "field" => matches!(entry.kind, SymbolKind::Field | SymbolKind::EnumMember),
            "property" => matches!(entry.kind, SymbolKind::Property | SymbolKind::Field),
            "event" => entry.kind == SymbolKind::Event,
            _ => false,
        }).map(|entry| entry.id.clone())
    }

    /// The function (or, failing that, any symbol) whose declaration contains `site`.
    fn owner(&self, site: &ByteRange) -> Option<&Entry> {
        self.innermost(&site.file, site.start, |e| e.has_flow)
            .or_else(|| self.innermost(&site.file, site.start, |_| true))
    }

    pub(crate) fn owner_id(&self, site: &ByteRange) -> Option<&str> {
        self.owner(site).map(|entry| entry.id.as_str())
    }

    fn owning_type(&self, site: &ByteRange) -> Option<&Entry> {
        self.innermost(&site.file, site.start, |e| e.kind.is_type())
    }

    pub(crate) fn type_at(&self, site: &ByteRange) -> Option<&str> {
        self.owning_type(site).map(|entry| entry.id.as_str())
    }

    fn kind_of(&self, id: &str) -> Option<SymbolKind> {
        self.kinds.get(id).copied()
    }
}

/// Structure edges derived from facts.
pub struct SemanticEdges {
    pub edges: Vec<StructEdge>,
    /// External symbols the edges point to (`ext:System.IDisposable`).
    pub externals: Vec<Symbol>,
    /// Types whose bases the facts describe; their name-based base edges should be dropped.
    pub typed_bases: HashSet<SymbolId>,
    /// Project types created at each construction site (`new X(...)`), for knowledge passes.
    pub constructs: Vec<(ByteRange, SymbolId)>,
    /// Every resolved call: site, calling function, called symbol.
    pub calls: Vec<(ByteRange, SymbolId, SymbolId)>,
}

pub fn edges(facts: &SemanticFacts, locator: &Locator, language: Language) -> SemanticEdges {
    let mut edges = BTreeSet::new();
    // Sorted by id, so the graph comes out the same on every run.
    let mut externals: BTreeMap<String, Symbol> = BTreeMap::new();
    let mut typed_bases = HashSet::new();
    let mut constructs = Vec::new();
    let mut calls = Vec::new();
    let mut by_site: HashMap<&ByteRange, Vec<(&RefKind, &SemanticTarget)>> = HashMap::new();

    for r in &facts.refs {
        by_site.entry(&r.site).or_default().push((&r.kind, &r.target));
        match r.kind {
            RefKind::Call => {
                if let (Some(owner), Some(target)) = (locator.owner(&r.site), locator.target(&r.target))
                    && owner.id != target {
                        calls.push((r.site.clone(), owner.id.clone(), target.clone()));
                        edges.insert((owner.id.clone(), target, StructEdgeKind::Calls));
                    }
            }
            RefKind::Construct => {
                let ty = locator.target(&r.target).and_then(|id| {
                    if locator.kind_of(&id).is_some_and(SymbolKind::is_type) {
                        Some(id)
                    } else {
                        // A constructor: its parent type.
                        r.target.decl.as_ref().and_then(|d| locator.owning_type(d)).map(|e| e.id.clone())
                    }
                });
                if let Some(ty) = &ty {
                    constructs.push((r.site.clone(), ty.clone()));
                }
                if let (Some(owner), Some(ty)) = (locator.owner(&r.site), ty) {
                    edges.insert((owner.id.clone(), ty, StructEdgeKind::Instantiates));
                }
            }
            RefKind::Base => {
                let Some(deriving) = locator.owning_type(&r.site) else { continue };
                typed_bases.insert(deriving.id.clone());
                let (to, contract) = match locator.target(&r.target) {
                    Some(id) => {
                        let contract = locator.kind_of(&id).is_some_and(SymbolKind::is_contract);
                        (id, contract)
                    }
                    None => {
                        let id = format!("ext:{}", r.target.display);
                        let short = r.target.display.rsplit('.').next().unwrap_or(&r.target.display).to_owned();
                        let contract = short.len() > 1
                            && short.starts_with('I')
                            && short.chars().nth(1).is_some_and(char::is_uppercase);
                        externals.entry(id.clone()).or_insert_with(|| external_type(&id, &r.target, contract, language));
                        (id, contract)
                    }
                };
                let kind = if contract { StructEdgeKind::Implements } else { StructEdgeKind::Inherits };
                edges.insert((deriving.id.clone(), to, kind));
            }
            RefKind::Raise => {
                if let (Some(owner), Some(event)) = (locator.owner(&r.site), locator.target(&r.target)) {
                    edges.insert((owner.id.clone(), event, StructEdgeKind::Raises));
                }
            }
            RefKind::Compatible => {
                if let (Some(ty), Some(target)) = (locator.owning_type(&r.site), locator.target(&r.target))
                    && ty.id != target
                {
                    edges.insert((ty.id.clone(), target, StructEdgeKind::Compatible));
                }
            }
            RefKind::Subscribe | RefKind::Unsubscribe | RefKind::Handler => {}
        }
    }

    // Subscriptions: event → handler method, or event → the function that registers a lambda.
    for (site, refs) in &by_site {
        let Some((_, event)) = refs.iter().find(|(k, _)| **k == RefKind::Subscribe) else { continue };
        let Some(event) = locator.target(event) else { continue };
        let handler = refs
            .iter()
            .find(|(k, _)| **k == RefKind::Handler)
            .and_then(|(_, t)| locator.target(t))
            .or_else(|| locator.owner(site).map(|e| e.id.clone()));
        if let Some(handler) = handler {
            edges.insert((event, handler, StructEdgeKind::Subscribes));
        }
    }

    SemanticEdges {
        edges: edges.into_iter().map(|(from, to, kind)| StructEdge::new(from, to, kind)).collect(),
        externals: externals.into_values().collect(),
        typed_bases,
        constructs,
        calls,
    }
}

fn external_type(id: &str, target: &SemanticTarget, contract: bool, language: Language) -> Symbol {
    Symbol {
        rust_contract: None,
        syntax_sites: None,
        type_info: None,
        value_info: None,
        id: id.to_owned(),
        kind: if contract { SymbolKind::Interface } else { SymbolKind::Class },
        name: target.display.rsplit('.').next().unwrap_or(&target.display).to_owned(),
        qualified_name: target.display.clone(),
        parent: None,
        language,
        origins: Vec::new(),
        flavors: Vec::new(),
        signature: None,
        type_text: None,
        has_flow: false,
        external: true,
        doc: None,
    }
}

fn within(outer_start: u32, outer_end: u32, r: &ByteRange) -> bool {
    outer_start <= r.start && r.end <= outer_end
}

/// Returns a copy of `flow` enriched with the facts that fall inside it.
pub fn enrich_flow(flow: &FlowGraph, facts: &SemanticFacts, locator: &Locator) -> FlowGraph {
    let refs: Vec<&SemanticRef> = facts.refs.iter().collect();
    let locals: Vec<&LocalTypeFact> = facts.locals.iter().collect();
    let mut flow = enrich_flow_with(flow, &refs, &locals, locator);
    annotate_navigation(&mut flow, facts.navigation.iter().flatten(), locator);
    annotate_expression_facts(&mut flow, facts.expressions.iter());
    annotate_value_facts(&mut flow, facts.values.iter(), &locals);
    annotate_types(&mut flow, facts.types.iter(), &locals);
    flow.attach_reading(&facts.reading);
    annotate_collection_roles(&mut flow, &refs, facts.binding_uses.iter());
    flow
}

pub(crate) fn annotate_value_facts<'a>(
    flow: &mut FlowGraph,
    facts: impl Iterator<Item = &'a ValueFact>,
    locals: &[&LocalTypeFact],
) {
    use nodex_ir::{ValueBasis, ValueInfo, ValueKind};
    let mut sites = HashMap::new();
    for fact in facts.filter(|f| f.site.file == flow.span.file && within(flow.span.start_byte, flow.span.end_byte, &f.site)) {
        let key = (fact.site.start, fact.site.end, fact.target);
        sites.entry(key).and_modify(|previous| {
            if *previous != fact.info {
                *previous = ValueInfo { kind: ValueKind::Unknown, basis: ValueBasis::TypeAnalysis, qualifier: None, passing: None };
            }
        }).or_insert(fact.info);
    }
    fn apply(e: &mut Expr, sites: &HashMap<(u32, u32, ValueFactTarget), ValueInfo>) {
        if let Some(info) = sites.get(&(e.span.start_byte, e.span.end_byte, ValueFactTarget::Expression)) {
            e.value_info = Some(*info);
        }
        for child in &mut e.children { apply(child, sites); }
    }
    let rust_types: HashMap<_, _> = if flow.language == Language::Rust {
        locals.iter().filter(|local| local.site.file == flow.span.file)
            .map(|local| ((local.site.start, local.site.end), local.ty.as_str())).collect()
    } else {
        HashMap::new()
    };
    for node in &mut flow.nodes {
        if let Some(expr) = &mut node.expr { apply(expr, &sites); }
        for pin in &mut node.outputs {
            let Some(span) = &pin.declaration else { continue };
            if let Some(info) = sites.get(&(span.start_byte, span.end_byte, ValueFactTarget::Binding)) {
                pin.value_info = Some(*info);
            } else if let Some(mut info) = rust_types.get(&(span.start_byte, span.end_byte))
                .and_then(|ty| nodex_lang_api::value_shapes::written_type(Language::Rust, ty)) {
                info.basis = ValueBasis::TypeAnalysis;
                pin.value_info = Some(info);
            }
        }
    }
    nodex_lang_api::value_shapes::carry_to_uses(flow);
}

pub(crate) fn annotate_member_values(graph: &mut nodex_ir::StructureGraph, facts: &[ValueFact], locator: &Locator) {
    let mut values = HashMap::new();
    for fact in facts.iter().filter(|f| f.target == ValueFactTarget::Member) {
        let Some(entry) = locator.innermost(&fact.site.file, fact.site.start, |entry|
            matches!(entry.kind, SymbolKind::Field | SymbolKind::Property | SymbolKind::Method | SymbolKind::Function))
        else { continue };
        values.entry(entry.id.as_str()).and_modify(|previous| {
            if *previous != Some(fact.info) { *previous = None; }
        }).or_insert(Some(fact.info));
    }
    for symbol in &mut graph.symbols {
        if let Some(info) = values.get(symbol.id.as_str()) {
            symbol.value_info = *info;
        }
    }
}

pub(crate) type TypeFactSites = HashMap<(u32, u32, ValueFactTarget), nodex_ir::types::TypeInfo>;

pub(crate) fn annotate_types<'a>(
    flow: &mut FlowGraph,
    facts: impl Iterator<Item = &'a nodex_ir::semantic::TypeFact>,
    locals: &[&LocalTypeFact],
) {
    use nodex_ir::types::{TypeInfo, TypeOrigin, TypeShape};
    let mut sites = TypeFactSites::new();
    for fact in facts.filter(|fact| fact.site.file == flow.span.file && within(flow.span.start_byte, flow.span.end_byte, &fact.site)) {
        sites.entry((fact.site.start, fact.site.end, fact.target))
            .and_modify(|previous| {
                if previous.shape != fact.info.shape {
                    *previous = TypeInfo::new(TypeShape::Unknown, TypeOrigin::Compiler, None);
                }
            }).or_insert_with(|| fact.info.clone());
    }
    crate::semantic_types::annotate(flow, &sites, locals);
}

pub(crate) fn annotate_member_types(graph: &mut nodex_ir::StructureGraph, facts: &[nodex_ir::semantic::TypeFact], locator: &Locator) {
    use nodex_ir::types::{TypeInfo, TypeOrigin, TypeShape};
    let mut types = HashMap::new();
    for fact in facts.iter().filter(|fact| fact.target == ValueFactTarget::Member) {
        let Some(entry) = locator.innermost(&fact.site.file, fact.site.start, |entry|
            matches!(entry.kind, SymbolKind::Field | SymbolKind::Property | SymbolKind::Method | SymbolKind::Function))
        else { continue };
        types.entry(entry.id.as_str()).and_modify(|previous: &mut TypeInfo| {
            if previous.shape != fact.info.shape {
                *previous = TypeInfo::new(TypeShape::Unknown, TypeOrigin::Compiler, None);
            }
        }).or_insert_with(|| fact.info.clone());
    }
    for symbol in &mut graph.symbols {
        if let Some(info) = types.get(symbol.id.as_str()) {
            symbol.type_info = Some(info.clone());
        }
    }
}

pub(crate) fn annotate_collection_roles<'a>(
    flow: &mut FlowGraph, refs: &[&SemanticRef], binding_uses: impl Iterator<Item = &'a BindingUseFact>,
) {
    crate::semantic_types::roles(flow, refs, binding_uses);
}

pub(crate) fn annotate_expression_facts<'a>(flow: &mut FlowGraph, facts: impl Iterator<Item = &'a ExpressionFact>) {
    let mut by_site: HashMap<(u32, u32), Vec<&ExpressionFact>> = HashMap::new();
    for fact in facts.filter(|fact| fact.site.file == flow.span.file && within(flow.span.start_byte, flow.span.end_byte, &fact.site)) {
        by_site.entry((fact.site.start, fact.site.end)).or_default().push(fact);
    }
    if by_site.is_empty() {
        return;
    }
    fn annotate(expr: &mut Expr, by_site: &HashMap<(u32, u32), Vec<&ExpressionFact>>) {
        if let Some(facts) = by_site.get(&(expr.span.start_byte, expr.span.end_byte)) {
            for fact in facts {
                if facts.iter().any(|other| other.kind == fact.kind && other.ty != fact.ty) {
                    continue;
                }
                let flavor = match fact.kind {
                    ExpressionFactKind::Narrowed => {
                        let Some(ty) = &fact.ty else { continue };
                        expr.ty = Some(ty.clone());
                        format!("narrowed:{ty}")
                    }
                    ExpressionFactKind::Boxing => "boxing".into(),
                    ExpressionFactKind::ValueCopy => "value-copy".into(),
                    ExpressionFactKind::ReferenceCopy => "reference-copy".into(),
                    ExpressionFactKind::Dynamic => "dynamic".into(),
                    ExpressionFactKind::Freeze => "runtime-freeze".into(),
                    ExpressionFactKind::BooleanAssignment => {
                        if expr.concept == Concept::Assign && matches!(expr.op.as_deref(), Some("bit_and" | "bit_or")) {
                            expr.op = None;
                        }
                        continue;
                    }
                    ExpressionFactKind::Spawn | ExpressionFactKind::Sleep | ExpressionFactKind::ChannelSend
                    | ExpressionFactKind::ProcessExit => continue,
                };
                if !expr.flavors.contains(&flavor) {
                    expr.flavors.push(flavor.clone());
                }
                let sites = expr.syntax_sites.get_or_insert_with(Vec::new);
                if !sites.iter().any(|site| site.key == flavor && site.span == expr.span) {
                    sites.push(nodex_ir::SyntaxSite { key: flavor, span: expr.span.clone(), pin: None });
                }
            }
        }
        for child in &mut expr.children {
            annotate(child, by_site);
        }
    }
    let mut process_exits = HashSet::new();
    for node in &mut flow.nodes {
        if let Some(expr) = &mut node.expr {
            if matches!(node.concept, Concept::Call | Concept::Var)
                && let Some(facts) = by_site.get(&(expr.span.start_byte, expr.span.end_byte))
            {
                for fact in facts {
                    match fact.kind {
                        ExpressionFactKind::Spawn => node.concept = Concept::Spawn,
                        ExpressionFactKind::Sleep => node.concept = Concept::Wait,
                        ExpressionFactKind::ChannelSend => node.concept = Concept::Emit,
                        ExpressionFactKind::ProcessExit => {
                            process_exits.insert(node.id.clone());
                            node.outputs.retain(|pin| pin.kind != nodex_ir::PinKind::Exec);
                        }
                        _ => {}
                    }
                }
            }
            annotate(expr, &by_site);
        }
    }
    if !process_exits.is_empty() {
        flow.edges.retain(|edge| edge.kind != nodex_ir::FlowEdgeKind::Exec || !process_exits.contains(&edge.from));
    }
}

/// Member expressions keep the compiler's target when their reference token is unambiguous.
pub(crate) fn annotate_navigation<'a>(
    flow: &mut FlowGraph,
    facts: impl Iterator<Item = &'a NavigationFact>,
    locator: &Locator,
) {
    let mut references: Vec<_> = facts.filter(|fact| {
        !fact.implementation && fact.site.file == flow.span.file &&
            within(flow.span.start_byte, flow.span.end_byte, &fact.site) &&
            matches!(fact.target.kind.as_str(), "field" | "property" | "event" | "variable")
    }).collect();
    if references.is_empty() {
        return;
    }
    references.sort_unstable_by_key(|fact| fact.site.start);
    for expr in flow.nodes.iter_mut().filter_map(|node| node.expr.as_mut()) {
        annotate_member(expr, &references, locator);
    }
}

fn annotate_member(expr: &mut Expr, references: &[&NavigationFact], locator: &Locator) {
    for child in &mut expr.children {
        annotate_member(child, references, locator);
    }
    if expr.symbol.is_some() || !matches!(expr.concept, Concept::Var | Concept::GetMember | Concept::Index) {
        return;
    }
    let start = expr.span.start_byte;
    let end = expr.span.end_byte;
    let first = references.partition_point(|fact| fact.site.start < start);
    let mut target = None;
    for fact in references[first..].iter().take_while(|fact| fact.site.start < end) {
        let site = &fact.site;
        if site.end > end || !member_token(expr, site) {
            continue;
        }
        let Some(id) = locator.navigation_target(&fact.target) else { return };
        if target.as_ref().is_some_and(|(previous, _)| previous != &id) {
            return;
        }
        target = Some((id, &fact.target.display));
    }
    if let Some((id, display)) = target {
        expr.symbol = Some(id);
        if expr.focus.is_some() {
            expr.target = Some(display.clone());
        }
    }
}

fn member_token(expr: &Expr, site: &ByteRange) -> bool {
    match expr.concept {
        Concept::Var => site.start == expr.span.start_byte && site.end == expr.span.end_byte,
        Concept::GetMember => expr.children.first().is_none_or(|receiver| receiver.span.end_byte <= site.start),
        Concept::Index => {
            let [receiver, argument, ..] = expr.children.as_slice() else { return false };
            receiver.span.end_byte <= site.start &&
                (site.end <= argument.span.start_byte ||
                    (argument.concept == Concept::Literal && site.start == argument.span.start_byte && site.end == argument.span.end_byte))
        }
        _ => false,
    }
}

/// [`enrich_flow`] over candidate facts (all of them, or those of the flow's file).
pub fn enrich_flow_with(flow: &FlowGraph, refs: &[&SemanticRef], locals: &[&LocalTypeFact], locator: &Locator) -> FlowGraph {
    let mut flow = flow.clone();
    let file = flow.span.file.clone();
    let language = flow.language;
    let (fs, fe) = (flow.span.start_byte, flow.span.end_byte);
    let refs: Vec<&SemanticRef> = refs.iter().copied().filter(|r| r.site.file == file && within(fs, fe, &r.site)).collect();

    for r in &refs {
        // The statement that contains the site: the smallest node span around it.
        let Some(idx) = flow
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| within(n.span.start_byte, n.span.end_byte, &r.site))
            .min_by_key(|(_, n)| n.span.end_byte - n.span.start_byte)
            .map(|(i, _)| i)
        else {
            continue;
        };
        let symbol = locator.target(&r.target);
        let node = &mut flow.nodes[idx];
        match r.kind {
            RefKind::Call | RefKind::Construct => {
                if !node.calls.iter().any(|call| match (&call.symbol, &symbol) {
                    (Some(existing), Some(target)) => existing == target,
                    (None, None) => call.display == r.target.display && call.external == r.target.external,
                    _ => false,
                }) {
                    node.calls.push(CallTarget {
                        symbol: symbol.clone(),
                        display: r.target.display.clone(),
                        external: r.target.external,
                    });
                }
                if let Some(expr) = node.expr.as_mut() {
                    annotate(expr, &r.site, symbol.as_deref(), &r.target, language);
                    if node.concept == Concept::Construct && expr.concept == Concept::Construct && node.focus.is_none() {
                        node.focus = expr.focus.clone();
                    }
                }
                if let Some(emit) = &r.emit_focus
                    && matches!(node.concept, Concept::Call | Concept::Emit)
                    && node.expr.as_ref().is_some_and(|expr| expr.concept == Concept::Call && invocation_matches(expr, &r.site, language))
                {
                    apply_emit_focus(node, emit);
                }
            }
            RefKind::Subscribe | RefKind::Unsubscribe => {
                if matches!(node.concept, Concept::Assign | Concept::SetMember | Concept::Subscribe) {
                    let op = if r.kind == RefKind::Subscribe { "+=" } else { "-=" };
                    if let Some((left, right)) = node.code.trim_end_matches(';').split_once(op) {
                        let (left, right) = (left.trim(), right.trim());
                        node.label = if r.kind == RefKind::Subscribe {
                            format!("on {left}, run {right}")
                        } else {
                            format!("stop running {right} on {left}")
                        };
                    }
                    node.concept = Concept::Subscribe;
                    node.flavors.retain(|f| f != "implicit-this" && f != "index");
                    if r.kind == RefKind::Unsubscribe && !node.flavors.iter().any(|f| f == "unsubscribe") {
                        node.flavors.push("unsubscribe".into());
                    }
                }
            }
            RefKind::Raise => {
                if let Some(expr) = node.expr.as_mut() {
                    let direct = expr.concept == Concept::Call && invocation_matches(expr, &r.site, language);
                    if direct && matches!(node.concept, Concept::Call | Concept::Emit) {
                        let focus = event_focus(node.focus.as_ref(), &r.target.display);
                        if node.concept == Concept::Call {
                            node.concept = Concept::Emit;
                            node.label = format!("raise {}", focus.primary);
                        }
                        expr.focus = Some(focus.clone());
                        node.focus = Some(focus);
                        expr.symbol = symbol;
                        expr.target = Some(r.target.display.clone());
                    } else {
                        annotate(expr, &r.site, symbol.as_deref(), &r.target, language);
                    }
                }
            }
            RefKind::Base | RefKind::Handler | RefKind::Compatible => {}
        }
    }

    // Inferred local types: fill `var:` pins, then the `use:` pins wired to them.
    let mut typed: Vec<(String, String, String)> = Vec::new();
    for local in locals.iter().copied().filter(|l| l.site.file == file && within(fs, fe, &l.site)) {
        let pin_id = format!("var:{}", local.name);
        for node in flow.nodes.iter_mut().filter(|n| within(n.span.start_byte, n.span.end_byte, &local.site)) {
            if let Some(pin) = node.outputs.iter_mut().find(|p| p.id == pin_id && p.kind == PinKind::Data) {
                // `{integer}`: a literal's type the frontend could not tell (Rust).
                if pin.ty.is_none() || pin.ty.as_deref().is_some_and(|t| t.starts_with('{')) {
                    pin.ty = Some(local.ty.clone());
                }
                typed.push((node.id.clone(), pin_id.clone(), pin.ty.clone().unwrap_or_default()));
            }
        }
    }
    for (node_id, pin_id, ty) in typed {
        let targets: Vec<(String, String)> = flow
            .edges
            .iter()
            .filter(|e| e.from == node_id && e.from_pin == pin_id)
            .map(|e| (e.to.clone(), e.to_pin.clone()))
            .collect();
        for (to, to_pin) in targets {
            if let Some(pin) = flow.nodes.iter_mut().find(|n| n.id == to).and_then(|n| n.inputs.iter_mut().find(|p| p.id == to_pin))
                && (pin.ty.is_none() || pin.ty.as_deref().is_some_and(|t| t.starts_with('{'))) {
                    pin.ty = Some(ty.clone());
                }
        }
    }
    flow
}

fn event_focus(source: Option<&Focus>, display: &str) -> Focus {
    let (owner, event) = display.rsplit_once('.').unwrap_or(("", display));
    source.filter(|f| f.primary.trim_start_matches('@') == event).cloned().unwrap_or_else(|| Focus {
        primary: event.to_owned(),
        prefix: (!owner.is_empty()).then(|| format!("{owner}.")),
        prefix_bytes: None,
        primary_bytes: None,
    })
}

fn apply_emit_focus(node: &mut FlowNode, emit: &EmitFocus) {
    let Some([receiver, arguments @ ..]) = node.expr.as_ref().map(|expr| expr.children.as_slice()) else { return };
    if !receiver.flavors.iter().any(|f| f == "target") {
        return;
    }
    let plain = || Focus {
        primary: emit.text.clone(),
        prefix: None,
        prefix_bytes: None,
        primary_bytes: (if emit.receiver { Some(receiver) } else { arguments.first() })
            .filter(|operand| operand.span.end_byte.saturating_sub(operand.span.start_byte) as usize == emit.text.len())
            .map(|operand| [operand.span.start_byte, operand.span.end_byte]),
    };
    let focus = if emit.receiver && matches!(receiver.concept, Concept::Var | Concept::GetMember | Concept::Index | Concept::SafeAccess) {
        receiver.focus.clone().unwrap_or_else(plain)
    } else {
        plain()
    };
    if node.concept != Concept::Emit || node.focus.as_ref() != Some(&focus) {
        let label = if emit.receiver {
            arguments.first().map_or_else(
                || format!("raise {}", emit.text),
                |arg| format!("send {} through {}", arg.label, emit.text),
            )
        } else {
            let receiver = receiver.focus.as_ref().map_or_else(
                || receiver.label.clone(),
                |focus| format!("{}{}", focus.prefix.as_deref().unwrap_or(""), focus.primary),
            );
            let mut label = format!("raise {} on {receiver}", emit.text);
            if arguments.len() > 1 {
                label.push_str(" with ");
                label.push_str(&arguments[1..].iter().map(|arg| arg.label.as_str()).collect::<Vec<_>>().join(", "));
            }
            label
        };
        node.concept = Concept::Emit;
        node.label = truncate(&normalize_ws(&label), 96);
        node.focus = Some(focus);
    }
    node.flavors.retain(|f| f != "angular:output");
    if emit.receiver {
        node.flavors.push("angular:output".into());
    }
}

fn invocation_matches(expr: &Expr, site: &ByteRange, language: Language) -> bool {
    expr.span.end_byte == site.end && (expr.span.start_byte == site.start ||
        // Roslyn starts `receiver?.Call()` at the member binding. Tree-sitter includes the receiver.
        language == Language::Csharp && expr.concept == Concept::Call && expr.span.start_byte < site.start &&
            expr.flavors.iter().any(|flavor| flavor == "safe-access"))
}

/// Sets the resolved target on the expression corresponding to the compiler's call site.
fn annotate(expr: &mut Expr, site: &ByteRange, symbol: Option<&str>, target: &SemanticTarget, language: Language) -> bool {
    if matches!(expr.concept, Concept::Call | Concept::Construct) && invocation_matches(expr, site, language) {
        expr.symbol = symbol.map(str::to_owned);
        expr.target = (target.kind != "unresolved").then(|| target.display.clone());
        if target.kind == "event" {
            expr.focus = Some(event_focus(expr.focus.as_ref(), &target.display));
        } else if expr.concept == Concept::Construct && expr.focus.is_none() && target.kind == "constructor"
            && let Some(ty) = &target.container
        {
            expr.focus = Some(Focus { primary: ty.clone(), prefix: None, prefix_bytes: None, primary_bytes: None });
        }
        let stage = expr.flavors.iter().any(|f| f == "stage");
        expr.flavors.retain(|f| f != "promise:all" && f != "option:map" && f != "option:and_then"
            && !f.starts_with("rust-iter:") && !f.starts_with("sequence:") && (!stage || f != "custom-stage"));
        let sequence = if target.external && symbol.is_none() {
            let display = target.display.trim_end_matches('.');
            let slice_method = display.strip_prefix("core::slice::")
                .and_then(|name| name.rsplit([':', ']']).next());
            match target.container.as_deref() {
                Some("Array" | "ReadonlyArray") if target.linq => Some("sequence:array"),
                Some("System.Linq.Enumerable") if stage && target.linq => Some("sequence:linq"),
                Some("Promise" | "PromiseLike") if target.linq => Some("sequence:promise"),
                Some("PromiseConstructor") if target.linq && target.display.starts_with("PromiseConstructor.all(") => Some("promise:all"),
                Some("Option") if target.display.starts_with("core::option::Option::map(") => Some("option:map"),
                Some("Option") if target.display.starts_with("core::option::Option::and_then(") => Some("option:and_then"),
                _ if slice_method == Some("iter()") => Some("rust-iter:borrow"),
                _ if slice_method == Some("iter_mut()") => Some("rust-iter:borrow-mut"),
                Some("Iterator") if target.display.starts_with("core::iter::") || target.display.starts_with("std::iter::") => {
                    Some("sequence:iterator")
                }
                Some(receiver) if language == Language::Rust && receiver != "IntoIterator"
                    && ["core::", "alloc::", "std::"].iter().any(|prefix| display.starts_with(prefix))
                    && display.ends_with("::into_iter()") => {
                    match receiver.strip_prefix('&') {
                        Some(reference) if reference.split_ascii_whitespace().any(|part| part == "mut") => Some("rust-iter:borrow-mut"),
                        Some(_) => Some("rust-iter:borrow"),
                        None => Some("rust-iter:by-value"),
                    }
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(sequence) = sequence {
            expr.flavors.push(sequence.into());
        } else if stage && !target.linq {
            expr.flavors.push("custom-stage".into());
        }
        return true;
    }
    expr.children.iter_mut().any(|c| annotate(c, site, symbol, target, language))
}
