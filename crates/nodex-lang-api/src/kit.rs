//! The lowering kit: helpers shared by every language frontend.

use std::collections::HashMap;

use nodex_ir::{
    Access, CallTarget, Concept, Expr, FlowEdge, FlowEdgeKind, FlowGraph, FlowNode, Language, NodeId, Pin, PinKind,
    Purity, Span, SymbolId, stable_id,
};

/// Maps byte offsets to 1-based line/column positions (columns in UTF-16 code units).
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, line_starts }
    }

    /// Returns `(line, column)`, both 1-based.
    pub fn position(&self, byte: usize) -> (u32, u32) {
        let byte = byte.min(self.text.len());
        let line = self.line_starts.partition_point(|&s| s <= byte) - 1;
        let start = self.line_starts[line];
        let col = self
            .text
            .get(start..byte)
            .map(|s| s.encode_utf16().count())
            .unwrap_or(byte - start);
        (line as u32 + 1, col as u32 + 1)
    }

    pub fn span(&self, file: &str, start_byte: usize, end_byte: usize) -> Span {
        let (start_line, start_col) = self.position(start_byte);
        let (end_line, end_col) = self.position(end_byte);
        Span {
            file: file.to_owned(),
            start_byte: start_byte as u32,
            end_byte: end_byte as u32,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// Entry label of a callback Blueprint named `<callee> callback` / `<event> handler`.
pub fn callback_entry_label(name: &str, params: &[&str]) -> String {
    let base = name.split('~').next().unwrap_or(name);
    let with = if params.is_empty() { String::new() } else { format!(" with ({})", params.join(", ")) };
    if let Some(callee) = base.strip_suffix(" callback") {
        format!("when {callee} calls this function{with}")
    } else if let Some(event) = base.strip_suffix(" handler") {
        format!("when {event} is raised{with}")
    } else {
        format!("when {base} is called{with}")
    }
}

/// Collapses all whitespace runs to single spaces and trims.
pub fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Shortens `text` to at most `max` characters, adding an ellipsis.
pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Everything needed to create a flow node.
#[derive(Debug, Clone)]
pub struct NodeSpec {
    pub concept: Concept,
    pub label: String,
    /// Statement header used both for display and as the stable-id anchor.
    pub code: String,
    pub span: Span,
    pub flavors: Vec<String>,
    pub expr: Option<Expr>,
    /// Pure nodes have no exec pins.
    pub purity: Purity,
    /// Targets known from syntax alone (for example a local function's own Blueprint).
    pub calls: Vec<CallTarget>,
}

impl NodeSpec {
    pub fn new(concept: Concept, label: impl Into<String>, code: impl Into<String>, span: Span) -> Self {
        Self {
            concept,
            label: label.into(),
            code: code.into(),
            span,
            flavors: Vec::new(),
            expr: None,
            purity: Purity::Impure,
            calls: Vec::new(),
        }
    }

    pub fn call(mut self, target: CallTarget) -> Self {
        self.calls.push(target);
        self
    }

    pub fn flavor(mut self, flavor: impl Into<String>) -> Self {
        let flavor = flavor.into();
        if !self.flavors.contains(&flavor) {
            self.flavors.push(flavor);
        }
        self
    }

    pub fn flavors(mut self, flavors: impl IntoIterator<Item = String>) -> Self {
        for f in flavors {
            self = self.flavor(f);
        }
        self
    }

    /// Sets the expression tree. Function values in it that have a Blueprint of their own
    /// (`Lambda` with a symbol) become call targets, so the node opens them.
    pub fn expr(mut self, expr: Option<Expr>) -> Self {
        if let Some(e) = &expr {
            let mut lambdas = Vec::new();
            collect_lambda_targets(e, &mut lambdas);
            for target in lambdas {
                if !self.calls.iter().any(|c| c.symbol == target.symbol) {
                    self.calls.push(target);
                }
            }
        }
        self.expr = expr;
        self
    }
}

fn collect_lambda_targets(e: &Expr, out: &mut Vec<CallTarget>) {
    if e.concept == Concept::Lambda
        && let Some(symbol) = &e.symbol
    {
        let display = symbol.rsplit_once('.').map(|(_, n)| n).unwrap_or(symbol).to_owned();
        out.push(CallTarget { symbol: Some(symbol.clone()), display, external: false });
    }
    for c in &e.children {
        collect_lambda_targets(c, out);
    }
}

/// An exec output that the next statement connects to.
pub type ExecCursor = (NodeId, String);

/// Builds a [`FlowGraph`]: nodes with stable ids, exec chains, and def→use data edges for locals.
pub struct FlowBuilder {
    symbol: SymbolId,
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
    ordinals: HashMap<(Concept, String), u32>,
    scopes: Vec<Vec<(String, NodeId, String)>>,
}

impl FlowBuilder {
    pub fn new(symbol: impl Into<SymbolId>) -> Self {
        Self {
            symbol: symbol.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            ordinals: HashMap::new(),
            scopes: vec![Vec::new()],
        }
    }

    /// Adds a node. Impure nodes other than entries get an `in` exec pin.
    pub fn add(&mut self, spec: NodeSpec) -> NodeId {
        let anchor = normalize_ws(&spec.code);
        let ordinal = self.ordinals.entry((spec.concept, anchor.clone())).or_insert(0);
        let id = stable_id(&self.symbol, spec.concept, &anchor, *ordinal);
        *ordinal += 1;

        let mut inputs = Vec::new();
        let is_entry = matches!(spec.concept, Concept::Entry | Concept::EventEntry);
        if spec.purity == Purity::Impure && !is_entry {
            inputs.push(Pin {
                id: "in".into(),
                kind: PinKind::Exec,
                label: String::new(),
                ty: None,
            });
        }
        self.nodes.push(FlowNode {
            id: id.clone(),
            concept: spec.concept,
            label: spec.label,
            code: truncate(&anchor, 160),
            span: spec.span,
            flavors: spec.flavors,
            purity: spec.purity,
            inputs,
            outputs: Vec::new(),
            expr: spec.expr,
            writes: Vec::new(),
            calls: spec.calls,
        });
        id
    }

    fn node_mut(&mut self, id: &str) -> &mut FlowNode {
        self.nodes
            .iter_mut()
            .find(|n| n.id == id)
            .expect("node exists in builder")
    }

    /// Adds an exec output pin and returns a cursor to it.
    pub fn exec_out(&mut self, node: &NodeId, pin: &str, label: &str) -> ExecCursor {
        let n = self.node_mut(node);
        if !n.outputs.iter().any(|p| p.id == pin) {
            n.outputs.push(Pin {
                id: pin.into(),
                kind: PinKind::Exec,
                label: label.into(),
                ty: None,
            });
        }
        (node.clone(), pin.to_owned())
    }

    /// Adds a data output pin for a local variable and defines it in the current scope.
    pub fn define(&mut self, node: &NodeId, name: &str, ty: Option<String>) {
        let pin = format!("var:{name}");
        let n = self.node_mut(node);
        if !n.outputs.iter().any(|p| p.id == pin) {
            n.outputs.push(Pin {
                id: pin.clone(),
                kind: PinKind::Data,
                label: name.into(),
                ty,
            });
        }
        if !n.writes.iter().any(|w| w == name) {
            n.writes.push(name.into());
        }
        self.scopes
            .last_mut()
            .expect("scope stack is never empty")
            .push((name.into(), node.clone(), pin));
    }

    /// Records that `node` writes an existing local (assignment, increment).
    pub fn write(&mut self, node: &NodeId, name: &str) {
        let n = self.node_mut(node);
        if !n.writes.iter().any(|w| w == name) {
            n.writes.push(name.into());
        }
    }

    /// Looks up the definition of `name` in the scope stack.
    pub fn lookup(&self, name: &str) -> Option<(NodeId, String)> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(n, _, _)| n == name)
            .map(|(_, node, pin)| (node.clone(), pin.clone()))
    }

    /// The type of the pin that currently defines local `name`, when known.
    pub fn type_of(&self, name: &str) -> Option<String> {
        let (node, pin) = self.lookup(name)?;
        self.nodes.iter().find(|n| n.id == node)?.outputs.iter().find(|p| p.id == pin)?.ty.clone()
    }

    /// True when the value defined at `node`/`pin` was moved away (a data wire that moves it).
    pub fn moved_from(&self, node: &str, pin: &str) -> bool {
        self.edges.iter().any(|e| e.from == node && e.from_pin == pin && e.access == Some(Access::Move))
    }

    /// Changes the label of a pin (Rust shadowing: `value₂`).
    pub fn set_pin_label(&mut self, node: &NodeId, pin: &str, label: &str) {
        let n = self.node_mut(node);
        if let Some(p) = n.outputs.iter_mut().chain(n.inputs.iter_mut()).find(|p| p.id == pin) {
            p.label = label.to_owned();
        }
    }

    /// Sets the type a pin carries. On the `catch:<i>` exec pins of a `Try` node it is the
    /// exception type the clause handles (`*` for every error); `nodex-core` reads it to link
    /// throws to the code that catches them.
    pub fn set_pin_type(&mut self, node: &NodeId, pin: &str, ty: &str) {
        let n = self.node_mut(node);
        if let Some(p) = n.outputs.iter_mut().chain(n.inputs.iter_mut()).find(|p| p.id == pin) {
            p.ty = Some(ty.to_owned());
        }
    }

    /// Connects the definition of local `name` (if any) to a data input on `node`.
    pub fn read(&mut self, node: &NodeId, name: &str) {
        self.read_with(node, name, None);
    }

    /// Like [`read`](Self::read), saying how the value is handed over (Rust ownership). A
    /// stronger access wins when the same local is read twice by one node
    /// (`BorrowMut` > `Move` > `Borrow` > `Copy`).
    pub fn read_with(&mut self, node: &NodeId, name: &str, access: Option<Access>) {
        let rank = |a: Option<Access>| match a {
            Some(Access::BorrowMut) => 4,
            Some(Access::Move) => 3,
            Some(Access::Borrow) => 2,
            Some(Access::Copy) => 1,
            None => 0,
        };
        let pin = format!("use:{name}");
        if let Some(e) = self.edges.iter_mut().find(|e| &e.to == node && e.to_pin == pin && e.kind == FlowEdgeKind::Data) {
            if rank(access) > rank(e.access) {
                e.access = access;
            }
            return;
        }
        self.read_inner(node, name, access);
    }

    fn read_inner(&mut self, node: &NodeId, name: &str, access: Option<Access>) {
        let Some((def_node, def_pin)) = self.lookup(name) else {
            return;
        };
        if &def_node == node {
            return;
        }
        let pin = format!("use:{name}");
        let def = self.nodes.iter().find(|n| n.id == def_node).and_then(|n| n.outputs.iter().find(|p| p.id == def_pin));
        let ty = def.and_then(|p| p.ty.clone());
        let label = def.map(|p| p.label.clone()).unwrap_or_else(|| name.to_owned());
        let n = self.node_mut(node);
        if n.inputs.iter().any(|p| p.id == pin) {
            return;
        }
        n.inputs.push(Pin {
            id: pin.clone(),
            kind: PinKind::Data,
            label,
            ty,
        });
        self.edges.push(FlowEdge {
            from: def_node,
            from_pin: def_pin,
            to: node.clone(),
            to_pin: pin,
            kind: FlowEdgeKind::Data,
            access,
        });
    }

    /// Connects an exec cursor to the `in` pin of `to`.
    pub fn link(&mut self, from: &ExecCursor, to: &NodeId) {
        self.edges.push(FlowEdge {
            from: from.0.clone(),
            from_pin: from.1.clone(),
            to: to.clone(),
            to_pin: "in".into(),
            kind: FlowEdgeKind::Exec,
            access: None,
        });
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Number of nodes created so far (use with [`FlowBuilder::add_flavor_at`]).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Adds a flavor to the node created at position `index`, if it exists.
    pub fn add_flavor_at(&mut self, index: usize, flavor: &str) {
        if let Some(n) = self.nodes.get_mut(index)
            && !n.flavors.iter().any(|f| f == flavor)
        {
            n.flavors.push(flavor.to_owned());
        }
    }

    /// Position of node `id` (for [`add_flavor_at`](Self::add_flavor_at)).
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    pub fn concept_of(&self, id: &str) -> Option<Concept> {
        self.nodes.iter().find(|n| n.id == id).map(|n| n.concept)
    }

    pub fn finish(self, name: impl Into<String>, language: Language, span: Span) -> FlowGraph {
        FlowGraph {
            symbol: self.symbol,
            name: name.into(),
            language,
            span,
            nodes: self.nodes,
            edges: self.edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_counts_utf16_columns() {
        let text = "ab\nçé x";
        let idx = LineIndex::new(text);
        assert_eq!(idx.position(0), (1, 1));
        assert_eq!(idx.position(3), (2, 1));
        // "çé" is 4 bytes but 2 UTF-16 units.
        assert_eq!(idx.position(3 + 4), (2, 3));
    }

    #[test]
    fn builder_assigns_ordinals_to_identical_statements() {
        let mut b = FlowBuilder::new("S.M()");
        let span = Span::default();
        let a = b.add(NodeSpec::new(Concept::Call, "f", "f();", span.clone()));
        let c = b.add(NodeSpec::new(Concept::Call, "f", "f();", span));
        assert_ne!(a, c);
    }

    #[test]
    fn read_creates_data_edge_from_definition() {
        let mut b = FlowBuilder::new("S.M()");
        let span = Span::default();
        let d = b.add(NodeSpec::new(Concept::Var, "x = 1", "int x = 1;", span.clone()));
        b.define(&d, "x", Some("int".into()));
        let u = b.add(NodeSpec::new(Concept::Call, "print", "print(x);", span.clone()));
        b.read(&u, "x");
        b.read(&u, "unknown");
        let g = b.finish("M", Language::Csharp, span);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].kind, FlowEdgeKind::Data);
        assert_eq!(g.node(&u).unwrap().inputs[1].ty.as_deref(), Some("int"));
    }
}
