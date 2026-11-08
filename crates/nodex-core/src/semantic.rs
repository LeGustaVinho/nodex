//! Joins semantic facts (keyed by byte ranges) with the syntax-only IR.
//!
//! - Structure: exact `Calls`, `Instantiates`, `Inherits`/`Implements`, `Raises` and
//!   `Subscribes` edges.
//! - Flows: resolved call targets on nodes and expressions, event subscriptions and raises
//!   promoted to `Subscribe`/`Emit`, inferred types on `var` data pins.

use std::collections::{BTreeSet, HashMap, HashSet};

use nodex_ir::semantic::{ByteRange, RefKind, SemanticFacts, SemanticTarget};
use nodex_ir::{
    CallTarget, Concept, Expr, FlowGraph, Language, PinKind, StructEdge, StructEdgeKind, Symbol, SymbolId, SymbolKind,
};

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
}

impl Locator {
    pub fn new(symbols: &[Symbol]) -> Self {
        let mut by_file: HashMap<String, Vec<Entry>> = HashMap::new();
        for s in symbols.iter().filter(|s| !s.external) {
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
        Self { by_file }
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

    /// The function (or, failing that, any symbol) whose declaration contains `site`.
    fn owner(&self, site: &ByteRange) -> Option<&Entry> {
        self.innermost(&site.file, site.start, |e| e.has_flow)
            .or_else(|| self.innermost(&site.file, site.start, |_| true))
    }

    fn owning_type(&self, site: &ByteRange) -> Option<&Entry> {
        self.innermost(&site.file, site.start, |e| e.kind.is_type())
    }

    fn kind_of(&self, id: &str) -> Option<SymbolKind> {
        self.by_file.values().flatten().find(|e| e.id == id).map(|e| e.kind)
    }
}

/// Structure edges derived from facts.
pub struct SemanticEdges {
    pub edges: Vec<StructEdge>,
    /// External symbols the edges point to (`ext:System.IDisposable`).
    pub externals: Vec<Symbol>,
    /// Types whose bases the facts describe; their name-based base edges should be dropped.
    pub typed_bases: HashSet<SymbolId>,
}

pub fn edges(facts: &SemanticFacts, locator: &Locator, language: Language) -> SemanticEdges {
    let mut edges = BTreeSet::new();
    let mut externals: HashMap<String, Symbol> = HashMap::new();
    let mut typed_bases = HashSet::new();
    let mut by_site: HashMap<&ByteRange, Vec<(&RefKind, &SemanticTarget)>> = HashMap::new();

    for r in &facts.refs {
        by_site.entry(&r.site).or_default().push((&r.kind, &r.target));
        match r.kind {
            RefKind::Call => {
                if let (Some(owner), Some(target)) = (locator.owner(&r.site), locator.target(&r.target))
                    && owner.id != target {
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
        edges: edges.into_iter().map(|(from, to, kind)| StructEdge { from, to, kind }).collect(),
        externals: externals.into_values().collect(),
        typed_bases,
    }
}

fn external_type(id: &str, target: &SemanticTarget, contract: bool, language: Language) -> Symbol {
    Symbol {
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
    let mut flow = flow.clone();
    let file = flow.span.file.clone();
    let (fs, fe) = (flow.span.start_byte, flow.span.end_byte);
    let refs: Vec<_> = facts.refs.iter().filter(|r| r.site.file == file && within(fs, fe, &r.site)).collect();

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
                if !node.calls.iter().any(|c| c.display == r.target.display) {
                    node.calls.push(CallTarget {
                        symbol: symbol.clone(),
                        display: r.target.display.clone(),
                        external: r.target.external,
                    });
                }
                if let Some(expr) = node.expr.as_mut() {
                    annotate(expr, &r.site, symbol.as_deref(), &r.target);
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
                if node.concept == Concept::Call {
                    node.concept = Concept::Emit;
                    node.label = format!("raise {}", r.target.display.rsplit('.').next().unwrap_or(&r.target.display));
                }
                if let Some(expr) = node.expr.as_mut() {
                    annotate(expr, &r.site, symbol.as_deref(), &r.target);
                }
            }
            RefKind::Base | RefKind::Handler | RefKind::Compatible => {}
        }
    }

    // Inferred local types: fill `var:` pins, then the `use:` pins wired to them.
    let mut typed: Vec<(String, String, String)> = Vec::new();
    for local in facts.locals.iter().filter(|l| l.site.file == file && within(fs, fe, &l.site)) {
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

/// Sets the resolved target on the expression whose span equals `site`.
fn annotate(expr: &mut Expr, site: &ByteRange, symbol: Option<&str>, target: &SemanticTarget) -> bool {
    if expr.span.start_byte == site.start && expr.span.end_byte == site.end && matches!(expr.concept, Concept::Call | Concept::Construct) {
        expr.symbol = symbol.map(str::to_owned);
        expr.target = Some(target.display.clone());
        if expr.flavors.iter().any(|f| f == "stage") {
            expr.flavors.retain(|f| f != "custom-stage");
            if !target.linq {
                expr.flavors.push("custom-stage".into());
            }
        }
        return true;
    }
    expr.children.iter_mut().any(|c| annotate(c, site, symbol, target))
}
