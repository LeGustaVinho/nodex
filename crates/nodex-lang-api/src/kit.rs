//! The lowering kit: helpers shared by every language frontend.

use std::collections::{HashMap, HashSet};

use nodex_ir::{
    Access, Binding, BindingForm, BindingOrigin, CallTarget, Concept, Expr, FlowEdge, FlowEdgeKind, FlowGraph, FlowNode, Focus, Language, NodeId, Pin, PinKind,
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
        let bytes = text.as_bytes();
        line_starts.extend(bytes.iter().enumerate().filter_map(|(i, &byte)| {
            (byte == b'\n' || (byte == b'\r' && bytes.get(i + 1) != Some(&b'\n'))).then_some(i + 1)
        }));
        Self { text, line_starts }
    }

    pub fn line_text(&self, line: u32) -> Option<&'a str> {
        let i = line.checked_sub(1)? as usize;
        let start = *self.line_starts.get(i)?;
        let end = self.line_starts.get(i + 1).copied().unwrap_or(self.text.len());
        Some(self.text[start..end].trim_end_matches(['\r', '\n']))
    }

    /// Returns `(line, column)`, both 1-based.
    pub fn position(&self, byte: usize) -> (u32, u32) {
        let byte = byte.min(self.text.len());
        let line = self.line_starts.partition_point(|&s| s <= byte) - 1;
        let start = self.line_starts[line];
        let col = self
            .text
            .get(start..byte)
            .map(|s| if line == 0 { s.strip_prefix('\u{feff}').unwrap_or(s) } else { s }.encode_utf16().count())
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
    pub focus: Option<Focus>,
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
            focus: None,
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

    pub fn focus(mut self, focus: Option<Focus>) -> Self {
        self.focus = focus;
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
                declaration: None,
                type_info: None,
                value_info: None,
                id: "in".into(),
                kind: PinKind::Exec,
                label: String::new(),
                ty: None,
                binding: None,
            });
        }
        let focus = spec.focus.or_else(|| {
            let expr = spec.expr.as_ref()?;
            if spec.concept == Concept::Call && expr.concept == Concept::Pipeline {
                return expr.children.last()?.focus.clone();
            }
            let same_action = match spec.concept {
                Concept::Assign | Concept::SetMember => expr.concept == Concept::Assign
                    || (expr.concept == Concept::UnaryOp && matches!(expr.label.as_str(), "++" | "--")),
                Concept::Subscribe => expr.concept == Concept::Assign,
                Concept::Macro => matches!(expr.concept, Concept::Call | Concept::Macro),
                Concept::Call => matches!(expr.concept, Concept::Call | Concept::Construct),
                Concept::Construct | Concept::Await
                | Concept::TypeCheck | Concept::Convert => spec.concept == expr.concept,
                _ => false,
            };
            if same_action { expr.focus.clone() } else { None }
        });
        self.nodes.push(FlowNode {
            syntax_sites: None,
            id: id.clone(),
            concept: spec.concept,
            label: spec.label,
            focus,
            code: truncate(&anchor, 160),
            span: spec.span,
            flavors: spec.flavors,
            purity: spec.purity,
            inputs,
            outputs: Vec::new(),
            expr: spec.expr,
            writes: Vec::new(),
            binding_writes: Vec::new(),
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
                declaration: None,
                type_info: None,
                value_info: None,
                id: pin.into(),
                kind: PinKind::Exec,
                label: label.into(),
                ty: None,
                binding: None,
            });
        }
        (node.clone(), pin.to_owned())
    }

    /// Adds a data output pin for a local variable and defines it in the current scope.
    pub fn define(&mut self, node: &NodeId, name: &str, ty: Option<String>) {
        let pin = format!("var:{name}");
        self.define_pin(node, pin, name, ty, None);
    }

    /// Define a local with source-proven declaration rules. Resolve the previous
    /// binding before inserting the new one into the scope.
    pub fn declare(&mut self, node: &NodeId, name: &str, ty: Option<String>, form: BindingForm, reassignable: bool, has_initializer: bool) {
        self.declare_on(node, format!("var:{name}"), name, ty, form, reassignable, has_initializer);
    }

    pub fn declare_on(&mut self, node: &NodeId, pin: String, name: &str, ty: Option<String>, form: BindingForm, reassignable: bool, has_initializer: bool) {
        // The initializer of a redeclaration runs on this path, but uses the existing
        // function binding. Keep later reads tied to that binding across branches.
        if form == BindingForm::Var && self.scopes[0].iter().any(|(existing, ..)| existing == name) {
            if has_initializer {
                self.write_output(node, name);
            }
            return;
        }
        let shadows = (form != BindingForm::Var).then(|| self.lookup(name)).flatten()
            .filter(|(old_node, old_pin)| old_node != node || old_pin != &pin)
            .map(|(node, pin)| BindingOrigin { node, pin });
        let binding = Binding { form, reassignable, has_initializer, shadows };
        self.define_pin(node, pin, name, ty, Some(binding));
    }

    fn define_pin(&mut self, node: &NodeId, pin: String, name: &str, ty: Option<String>, binding: Option<Binding>) {
        let function_scoped = binding.as_ref().is_some_and(|rule| rule.form == BindingForm::Var);
        let n = self.node_mut(node);
        if !n.outputs.iter().any(|p| p.id == pin) {
            n.outputs.push(Pin {
                declaration: None,
                type_info: None,
                value_info: None,
                id: pin.clone(),
                kind: PinKind::Data,
                label: name.into(),
                ty,
                binding,
            });
        }
        if !n.writes.iter().any(|w| w == name) {
            n.writes.push(name.into());
        }
        let scope = if function_scoped { &mut self.scopes[0] } else { self.scopes.last_mut().expect("scope stack is never empty") };
        scope.push((name.into(), node.clone(), pin));
    }

    /// Records that `node` writes an existing local (assignment, increment).
    pub fn write(&mut self, node: &NodeId, name: &str) {
        let definition = self.lookup(name).map(|(node, pin)| BindingOrigin { node, pin });
        let n = self.node_mut(node);
        if !n.writes.iter().any(|w| w == name) {
            n.writes.push(name.into());
        }
        if let Some(definition) = definition
            && !n.binding_writes.contains(&definition)
        {
            n.binding_writes.push(definition);
        }
    }

    /// Expose an updated local as a result without changing its lexical definition.
    pub fn write_output(&mut self, node: &NodeId, name: &str) {
        let ty = self.type_of(name);
        self.write(node, name);
        let pin = format!("write:{name}");
        let n = self.node_mut(node);
        if !n.outputs.iter().any(|p| p.id == pin) {
            n.outputs.push(Pin { declaration: None, type_info: None, value_info: None, id: pin, kind: PinKind::Data, label: name.into(), ty, binding: None });
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

    /// Whether a local is still initialized on a cleanup path. A structured join may
    /// hide writes or moves on its arms, so that case can remain conditional.
    pub fn initialized_before(&self, binding: &ExecCursor, at: &ExecCursor, initially: bool) -> Option<bool> {
        let moves: HashSet<_> = self.edges.iter().filter(|e| {
            e.from == binding.0 && e.from_pin == binding.1 && e.access == Some(Access::Move)
                && self.nodes.iter().find(|node| node.id == e.to)
                    .is_some_and(|node| !node.flavors.iter().any(|flavor| flavor == "implicit-drop"))
        }).map(|e| e.to.as_str()).collect();
        let writes_binding = |node: &FlowNode| node.binding_writes.iter()
            .any(|origin| origin.node == binding.0 && origin.pin == binding.1);
        let writes = self.nodes.iter().any(writes_binding);
        if moves.is_empty() && !writes {
            return Some(initially);
        }
        let mut cursor = (at.0.as_str(), at.1.as_str());
        loop {
            let node = self.nodes.iter().find(|node| node.id == cursor.0)?;
            if node.id == binding.0 {
                return Some(initially);
            }
            if cursor.1 == "out" && node.outputs.iter().any(|pin| {
                pin.kind == PinKind::Exec && !matches!(pin.id.as_str(), "out" | "error")
            }) {
                return None;
            }
            if writes_binding(node) {
                return Some(true);
            }
            if moves.contains(node.id.as_str()) {
                return Some(false);
            }
            let mut predecessors = self.edges.iter().filter(|edge| edge.kind == FlowEdgeKind::Exec && edge.to == node.id);
            let edge = predecessors.next()?;
            if predecessors.next().is_some() {
                return None;
            }
            // Exec edges point from earlier nodes during lowering.
            cursor = (&edge.from, &edge.from_pin);
        }
    }

    /// Changes the presentation while keeping the original statement ID for navigation and replay.
    pub fn relabel(&mut self, node: &str, concept: Concept, label: String) {
        let node = self.node_mut(node);
        node.concept = concept;
        node.label = label;
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
            Some(Access::Move | Access::ByValue) => 3,
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
        self.read_binding(node, name, (&def_node, &def_pin), access);
    }

    /// Reads an exact binding even when a newer local with the same name is in scope.
    pub fn read_binding(&mut self, node: &NodeId, name: &str, binding: (&str, &str), access: Option<Access>) {
        let (def_node, def_pin) = binding;
        if def_node == node {
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
            declaration: None,
            type_info: None,
            value_info: None,
            id: pin.clone(),
            kind: PinKind::Data,
            label,
            ty,
            binding: None,
        });
        self.edges.push(FlowEdge {
            from: def_node.to_owned(),
            from_pin: def_pin.to_owned(),
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

    /// ID of a node at an earlier `node_count()` checkpoint, if lowering created one.
    pub fn node_id_at(&self, index: usize) -> Option<&str> {
        self.nodes.get(index).map(|node| node.id.as_str())
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
            source_revision: None,
            nodes: self.nodes,
            edges: self.edges,
            reading: None,
            borrows: None,
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
    fn line_index_handles_cr_crlf_and_lf() {
        let text = "one\rtwó\r\nthree\nfour";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text.find('ó').unwrap()), (2, 3));
        assert_eq!(index.position(text.find("three").unwrap()), (3, 1));
        assert_eq!(index.position(text.find("four").unwrap()), (4, 1));
        assert_eq!(index.line_text(2), Some("twó"));
        assert_eq!(index.line_text(3), Some("three"));
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
