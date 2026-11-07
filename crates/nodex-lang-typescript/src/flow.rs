//! Function bodies → Blueprint flow graphs.
//!
//! Same conventions as every language (see the "Visual Language" doc): one impure node per
//! statement with its expression tree kept for on-demand expansion; compound statements own
//! named exec outputs (`true`, `body`, `try`, `case:N`) and resume at `out`.

use std::collections::HashMap;

use nodex_ir::{CallTarget, Concept, Expr, FlowGraph, Language, SymbolId, TypeRef, TypeRelation};
use nodex_lang_api::kit::{ExecCursor, FlowBuilder, NodeSpec, truncate};
use tree_sitter::Node;

use crate::expr::{ExprLowering, PipelineKind, reads, unparen};
use crate::frameworks::{self, Frameworks};
use crate::structure::{BodyJob, BodyKind};
use crate::syntax::{Src, children, first_child_of_kind, is_function_value, is_statement, named_children, parameters, pattern_names, string_value};

pub struct Lowered {
    pub graph: FlowGraph,
    pub type_refs: Vec<TypeRef>,
}

type Flow = Option<ExecCursor>;

struct Lowering<'a, 'f> {
    src: Src<'a>,
    b: FlowBuilder,
    ex: ExprLowering<'a>,
    /// Functions that have their own Blueprint, by start byte.
    functions: &'f HashMap<usize, SymbolId>,
    label_max: usize,
    fw: Frameworks,
    /// React state setters declared in this body (`const [n, setN] = useState(0)` → `setN`).
    setters: Vec<String>,
    /// The body is a React component: its returned JSX is what it renders.
    component: bool,
}

const ENTRY_FLAVORS: &[&str] = &[
    "async", "generator", "static", "abstract", "override", "getter", "setter", "top-level", "arrow", "bound",
    "export", "default-export", "local-function", "private", "prototype", "commonjs",
];

/// Event subscription methods (`emitter.on("x", f)`, `el.addEventListener("click", f)`).
const SUBSCRIBE: &[&str] = &["on", "once", "addListener", "addEventListener", "prependListener", "prependOnceListener"];
const UNSUBSCRIBE: &[&str] = &["off", "removeListener", "removeEventListener", "removeAllListeners"];
const EMIT: &[&str] = &["emit", "dispatchEvent"];

pub fn lower_body(src: Src, job: &BodyJob, functions: &HashMap<usize, SymbolId>, fw: Frameworks) -> Lowered {
    let mut l = Lowering {
        src,
        b: FlowBuilder::new(job.symbol.clone()),
        ex: ExprLowering { functions: functions.clone(), ..ExprLowering::new(src) },
        functions,
        label_max: 96,
        fw,
        setters: Vec::new(),
        component: job.flavors.iter().any(|f| f == "react:component")
            || (job.name == "render" && job.flavors.iter().any(|f| f == "react:lifecycle")),
    };

    let params: Vec<&str> = job.params.iter().map(|(n, _)| n.as_str()).collect();
    let label = match job.kind {
        BodyKind::Constructor => format!("when a new {} is created", job.name),
        BodyKind::Getter => format!("when {} is read", job.name.trim_end_matches(".get")),
        BodyKind::Setter => format!("when {} is assigned", job.name.trim_end_matches(".set")),
        BodyKind::StaticBlock => format!("when class {} is defined", job.name),
        BodyKind::TopLevel => "when the module is loaded".to_owned(),
        BodyKind::LocalFunction if job.flavors.iter().any(|f| f == "callback") => {
            nodex_lang_api::kit::callback_entry_label(&job.name, &params)
        }
        BodyKind::LocalFunction => format!("when local function {}({}) is called", job.name, params.join(", ")),
        BodyKind::Function if job.flavors.iter().any(|f| f == "iife") => "when the module runs it, right where it is written".to_owned(),
        BodyKind::Function | BodyKind::Method => format!("when {}({}) is called", job.name, params.join(", ")),
    };
    let entry_span = if job.kind == BodyKind::TopLevel {
        src.index.span(src.file, 0, 0)
    } else {
        src.header_span(job.decl, Some(job.body))
    };
    let flavors = job.flavors.iter().filter(|f| ENTRY_FLAVORS.contains(&f.as_str()) || f.starts_with("attr:")).cloned();
    // A framework calls this body (lifecycle, render, registered callback): an event entry.
    let (concept, label) = match &job.event {
        Some(event) => (Concept::EventEntry, event.clone()),
        None => (Concept::Entry, label),
    };
    let flavors = flavors.chain(job.flavors.iter().filter(|f| f.contains(':') && !f.starts_with("attr:")).cloned());
    let entry = l.b.add(NodeSpec::new(concept, label, job.signature.clone(), entry_span).flavors(flavors));
    let start = l.b.exec_out(&entry, "out", "");
    for (name, ty) in &job.params {
        if !name.is_empty() {
            l.b.define(&entry, name, ty.clone());
        }
    }

    match job.kind {
        BodyKind::TopLevel => {
            l.statements(&job.stmts, Some(start));
        }
        _ if job.body.kind() == "statement_block" => {
            l.stmt(job.body, Some(start));
        }
        _ => {
            // Arrow function with an expression body: `(x) => x * 2`.
            if job.returns_void {
                l.expression_statement(job.body, job.body, Some(start));
            } else {
                l.return_value(job.body, Some(job.body), Some(start));
            }
        }
    }

    let type_refs = l
        .ex
        .instantiated
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| TypeRef { from: job.symbol.clone(), name: n.clone(), relation: TypeRelation::Instantiates })
        .collect();
    let graph = l.b.finish(job.name.clone(), Language::Typescript, src.span(job.decl));
    Lowered { graph, type_refs }
}

/// True if `expr` (anywhere) carries `flavor`.
fn has_flavor(expr: &Expr, flavor: &str) -> bool {
    expr.flavors.iter().any(|f| f == flavor) || expr.children.iter().any(|c| has_flavor(c, flavor))
}

impl Lowering<'_, '_> {
    fn label(&self, text: String) -> String {
        truncate(&text, self.label_max)
    }

    /// Adds a node and links it after `cur`. Unreachable statements get a flavor instead.
    fn place(&mut self, mut spec: NodeSpec, cur: &Flow) -> String {
        if cur.is_none() {
            spec = spec.flavor("unreachable");
        }
        if let Some(e) = &spec.expr
            && has_flavor(e, "loose-equality")
        {
            spec = spec.flavor("loose-equality");
        }
        let id = self.b.add(spec);
        if let Some(c) = cur {
            self.b.link(c, &id);
        }
        id
    }

    fn read_all(&mut self, id: &String, node: Option<Node>) {
        if let Some(n) = node {
            for name in reads(self.src, n) {
                self.b.read(id, &name);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn simple(&mut self, s: Node, concept: Concept, label: String, expr: Option<Node>, flavors: Vec<String>, cur: Flow) -> Flow {
        let lowered = expr.map(|e| self.ex.lower(e));
        let spec = NodeSpec::new(concept, self.label(label), self.src.norm(s), self.src.span(s)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, expr);
        if concept.is_terminal() { None } else { Some(self.b.exec_out(&id, "out", "")) }
    }

    fn statements(&mut self, stmts: &[Node], mut cur: Flow) -> Flow {
        for s in stmts {
            cur = self.stmt(*s, cur);
        }
        cur
    }

    fn body(&mut self, s: Option<Node>, cur: Flow) -> Flow {
        let Some(s) = s else { return cur };
        self.b.push_scope();
        let end = self.stmt(s, cur);
        self.b.pop_scope();
        end
    }

    fn stmt(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        match s.kind() {
            "statement_block" => {
                let stmts: Vec<Node> = named_children(s).into_iter().filter(|c| is_statement(*c)).collect();
                self.b.push_scope();
                let end = self.statements(&stmts, cur);
                self.b.pop_scope();
                end
            }
            "empty_statement" | "comment" | "import_statement" | "import_alias" | "interface_declaration" | "type_alias_declaration"
            | "ambient_declaration" | "function_signature" => cur,
            "expression_statement" => match named_children(s).into_iter().next() {
                Some(e) if matches!(e.kind(), "string" | "internal_module" | "module") => cur,
                Some(e) => self.expression_statement(s, e, cur),
                None => cur,
            },
            "lexical_declaration" | "variable_declaration" => self.local_declaration(s, s, Vec::new(), cur),
            "export_statement" => self.export_statement(s, cur),
            "if_statement" => self.if_statement(s, cur),
            "switch_statement" => self.switch_statement(s, cur),
            "for_statement" => self.for_statement(s, cur),
            "for_in_statement" => self.for_in_statement(s, cur),
            "while_statement" | "do_statement" => self.while_statement(s, cur),
            "return_statement" => {
                let e = named_children(s).into_iter().next();
                self.return_value(s, e, cur)
            }
            "throw_statement" => {
                let e = named_children(s).into_iter().next();
                let label = format!("fail with {}", e.map(|e| self.ex.natural(e)).unwrap_or_default());
                self.simple(s, Concept::Throw, label, e, Vec::new(), cur)
            }
            "break_statement" | "continue_statement" => {
                let label_name = s.child_by_field_name("label").map(|l| src.norm(l));
                let (concept, text) = if s.kind() == "break_statement" {
                    (Concept::Break, "leave the loop")
                } else {
                    (Concept::Continue, "skip to the next iteration")
                };
                let (label, flavors) = match (label_name, s.kind()) {
                    (Some(l), "break_statement") => (format!("leave {l}"), vec![format!("label:{l}")]),
                    (Some(l), _) => (format!("continue with the next iteration of {l}"), vec![format!("label:{l}")]),
                    (None, _) => (text.to_owned(), Vec::new()),
                };
                self.simple(s, concept, label, None, flavors, cur)
            }
            "labeled_statement" => {
                let label = s.child_by_field_name("label").map(|l| src.norm(l));
                let Some(inner) = s.child_by_field_name("body") else { return cur };
                let before = self.b.node_count();
                let end = self.stmt(inner, cur);
                if let Some(l) = label {
                    self.b.add_flavor_at(before, &format!("label:{l}"));
                }
                end
            }
            "try_statement" => self.try_statement(s, cur),
            "debugger_statement" => self.simple(s, Concept::Call, "pause in the debugger".into(), None, vec!["debugger".into()], cur),
            "with_statement" => {
                let object = s.child_by_field_name("object").map(unparen);
                let body = s.child_by_field_name("body");
                let label = format!("with the members of {}", object.map(|o| self.ex.natural(o)).unwrap_or_default());
                let lowered = object.map(|o| self.ex.lower(o));
                let spec = NodeSpec::new(Concept::Scope, self.label(label), src.header(s, body), src.header_span(s, body))
                    .flavor("with")
                    .expr(lowered);
                let id = self.place(spec, &cur);
                self.read_all(&id, object);
                let b = self.b.exec_out(&id, "body", "Body");
                self.body(body, Some(b));
                Some(self.b.exec_out(&id, "out", "Then"))
            }
            "function_declaration" | "generator_function_declaration" => {
                let name = s.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                self.define_function(s, s, &name, cur)
            }
            "class_declaration" | "abstract_class_declaration" => {
                let name = s.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let body = s.child_by_field_name("body");
                let spec = NodeSpec::new(Concept::Var, format!("define class {name}"), src.header(s, body), src.header_span(s, body))
                    .flavor("class");
                let id = self.place(spec, &cur);
                self.b.define(&id, &name, None);
                Some(self.b.exec_out(&id, "out", ""))
            }
            "enum_declaration" => {
                let name = s.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let body = s.child_by_field_name("body");
                let spec = NodeSpec::new(Concept::Var, format!("define enum {name}"), src.header(s, body), src.header_span(s, body))
                    .flavor("enum");
                let id = self.place(spec, &cur);
                self.b.define(&id, &name, None);
                Some(self.b.exec_out(&id, "out", ""))
            }
            _ => {
                let label = src.norm(s);
                self.simple(s, Concept::Opaque, label, None, vec![format!("syntax:{}", s.kind())], cur)
            }
        }
    }

    /// A named function with its own Blueprint, shown where it is defined.
    fn define_function(&mut self, code: Node, func: Node, name: &str, cur: Flow) -> Flow {
        let src = self.src;
        let params: Vec<String> = parameters(src, func).into_iter().map(|(n, _)| n).collect();
        let body = func.child_by_field_name("body");
        let mut spec = NodeSpec::new(
            Concept::Lambda,
            self.label(format!("define function {name}({})", params.join(", "))),
            src.header(code, body),
            src.header_span(code, body),
        )
        .flavor("local-function");
        if let Some(id) = self.functions.get(&func.start_byte()) {
            spec = spec.call(CallTarget { symbol: Some(id.clone()), display: format!("{name}({})", params.join(", ")), external: false });
        }
        let id = self.place(spec, &cur);
        self.b.define(&id, name, None);
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn export_statement(&mut self, s: Node, cur: Flow) -> Flow {
        if let Some(decl) = s.child_by_field_name("declaration") {
            return match decl.kind() {
                "lexical_declaration" | "variable_declaration" => self.local_declaration(s, decl, vec!["export".into()], cur),
                _ => self.stmt(decl, cur),
            };
        }
        let Some(value) = s.child_by_field_name("value") else { return cur };
        let label = format!("export {} as the default", self.ex.natural(value));
        self.simple(s, Concept::Assign, label, Some(value), vec!["export".into(), "default-export".into()], cur)
    }

    fn local_declaration(&mut self, code: Node, decl: Node, extra: Vec<String>, mut cur: Flow) -> Flow {
        let src = self.src;
        let keyword = children(decl).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
        let mut flavors = extra;
        if keyword == "var" {
            flavors.push("var".into());
        } else if keyword == "const" {
            flavors.push("const".into());
        }
        let declarators: Vec<Node> = named_children(decl).into_iter().filter(|c| c.kind() == "variable_declarator").collect();
        let single = declarators.len() == 1;
        for d in declarators {
            let code_node = if single { code } else { d };
            let Some(name_node) = d.child_by_field_name("name") else {
                cur = self.simple(d, Concept::Opaque, src.norm(d), None, vec!["syntax:variable_declarator".into()], cur);
                continue;
            };
            let ty = d.child_by_field_name("type").map(|t| crate::syntax::type_text(src, t));
            let value = d.child_by_field_name("value");
            let bare = value.map(unparen);

            // `const f = () => { … }` with its own Blueprint.
            if let Some(v) = bare
                && is_function_value(v.kind())
                && self.functions.contains_key(&v.start_byte())
            {
                cur = self.define_function(code_node, v, &src.norm(name_node), cur);
                continue;
            }

            // `const server = app.listen(3000, () => { … })`: a registration whose result is kept.
            if let Some(v) = bare.filter(|v| v.kind() == "call_expression")
                && let Some((label, flavor, target)) = self.registration_value(v)
            {
                let names_text = src.norm(name_node);
                let mut spec = NodeSpec::new(Concept::Var, self.label(format!("{names_text} = {label}")), src.norm(code_node), src.span(code_node))
                    .flavors(flavors.clone())
                    .flavor(flavor)
                    .expr(Some(self.ex.lower(v)));
                if let Some(t) = target {
                    spec = spec.call(t);
                }
                let id = self.place(spec, &cur);
                self.read_all(&id, value);
                let mut names = Vec::new();
                pattern_names(src, name_node, &mut names);
                for n in names {
                    self.b.define(&id, &n, None);
                }
                cur = Some(self.b.exec_out(&id, "out", ""));
                continue;
            }

            // `const [n, setN] = useState(0)`, `const q = ref('')`, `const http = inject(HttpClient)`.
            let mut names = Vec::new();
            pattern_names(src, name_node, &mut names);
            if let Some(v) = bare
                && let Some((label, flavor)) = frameworks::declaration(src, self.fw, &names, v)
            {
                if flavor == "react:state"
                    && let Some(setter) = names.get(1)
                {
                    self.setters.push(setter.clone());
                }
                let mut spec = NodeSpec::new(Concept::Var, self.label(label), src.norm(code_node), src.span(code_node))
                    .flavors(flavors.clone())
                    .flavor(flavor)
                    .expr(value.map(|v| self.ex.lower(v)));
                // `useCallback(() => { … })`: the function has its own Blueprint.
                if let Some(inner) = v.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next()).map(unparen)
                    && let Some(id) = self.functions.get(&inner.start_byte())
                {
                    spec = spec.call(CallTarget { symbol: Some(id.clone()), display: names.join(", "), external: false });
                }
                let id = self.place(spec, &cur);
                self.read_all(&id, value);
                for n in &names {
                    self.b.define(&id, n, None);
                }
                cur = Some(self.b.exec_out(&id, "out", ""));
                continue;
            }

            if name_node.kind() != "identifier" {
                let mut names = Vec::new();
                pattern_names(src, name_node, &mut names);
                let mut fl = flavors.clone();
                let concept = if bare.is_some_and(|v| v.kind() == "await_expression") {
                    fl.push("destructure".into());
                    Concept::Await
                } else {
                    Concept::Destructure
                };
                let label = format!("unpack {} from {}", src.norm(name_node), value.map(|v| self.ex.natural(v)).unwrap_or_default());
                let lowered = value.map(|v| self.ex.lower(v));
                let spec = NodeSpec::new(concept, self.label(label), src.norm(code_node), src.span(code_node)).flavors(fl).expr(lowered);
                let id = self.place(spec, &cur);
                self.read_all(&id, value);
                for n in names {
                    self.b.define(&id, &n, None);
                }
                cur = Some(self.b.exec_out(&id, "out", ""));
                continue;
            }

            let name = src.norm(name_node);
            let mut fl = flavors.clone();
            if ty.as_deref() == Some("never") {
                let label = format!("check while compiling that every case of {} was handled", value.map(|v| self.ex.natural(v)).unwrap_or_default());
                cur = self.simple(code_node, Concept::TypeCheck, label, value, vec!["exhaustive-check".into()], cur);
                continue;
            }
            let (concept, label) = match bare {
                Some(v) if v.kind() == "await_expression" => {
                    let inner = named_children(v).into_iter().next().unwrap_or(v);
                    (Concept::Await, format!("{name} = wait for {}", self.ex.natural(inner)))
                }
                Some(v) => {
                    match self.ex.pipeline(v).map(|p| p.kind) {
                        Some(PipelineKind::Array) => fl.push("array-pipeline".into()),
                        Some(PipelineKind::Promise) => fl.push("promise".into()),
                        Some(PipelineKind::Rx) => fl.push("rx-pipeline".into()),
                        None => {}
                    }
                    if v.kind() == "call_expression" && v.child_by_field_name("function").is_some_and(|f| src.text(f) == "require") {
                        fl.push("commonjs".into());
                    }
                    (Concept::Var, format!("{name} = {}", self.ex.natural(v)))
                }
                None => (Concept::Var, format!("declare {name}")),
            };
            let lowered = value.map(|v| self.ex.lower(v));
            let spec = NodeSpec::new(concept, self.label(label), src.norm(code_node), src.span(code_node)).flavors(fl).expr(lowered);
            let id = self.place(spec, &cur);
            self.read_all(&id, value);
            self.b.define(&id, &name, ty);
            cur = Some(self.b.exec_out(&id, "out", ""));
        }
        cur
    }

    fn expression_statement(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let e = unparen(e);
        match e.kind() {
            "assignment_expression" if src.text(e).starts_with("using ") => self.using_declaration(s, e, false, cur),
            "assignment_expression" | "augmented_assignment_expression" => self.assignment(s, e, cur),
            "update_expression" => {
                let op = children(e).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
                let target = e.child_by_field_name("argument");
                let target_text = target.map(|t| src.norm(t)).unwrap_or_default();
                let label = if op == "++" { format!("increase {target_text} by 1") } else { format!("decrease {target_text} by 1") };
                let concept = if target.is_some_and(|t| t.kind() == "identifier") { Concept::Assign } else { Concept::SetMember };
                let flow = self.simple(s, concept, label, Some(e), Vec::new(), cur);
                if let (Some(t), Some((id, _))) = (target, &flow)
                    && t.kind() == "identifier"
                {
                    let id = id.clone();
                    self.b.write(&id, &src.norm(t));
                }
                flow
            }
            "call_expression" => self.call(s, e, cur),
            "await_expression" => {
                let inner = named_children(e).into_iter().next().unwrap_or(e);
                if unparen(inner).kind() == "assignment_expression" && src.text(inner).starts_with("using ") {
                    return self.using_declaration(s, unparen(inner), true, cur);
                }
                if let Some(label) = self.wait_label(inner) {
                    return self.simple(s, Concept::Wait, label, Some(e), Vec::new(), cur);
                }
                let label = format!("wait for {}", self.ex.natural(inner));
                self.simple(s, Concept::Await, label, Some(e), Vec::new(), cur)
            }
            "yield_expression" => {
                let value = named_children(e).into_iter().next();
                let delegate = children(e).iter().any(|c| src.text(*c) == "*");
                let label = match (value, delegate) {
                    (Some(v), true) => format!("produce every value of {}", self.ex.natural(v)),
                    (Some(v), false) => format!("produce {}", self.ex.natural(v)),
                    (None, _) => "produce undefined".to_owned(),
                };
                let flavors = if delegate { vec!["delegate".to_owned()] } else { Vec::new() };
                self.simple(s, Concept::Yield, label, value, flavors, cur)
            }
            "new_expression" if self.fw.node && e.child_by_field_name("constructor").is_some_and(|c| src.norm(c).ends_with("Worker")) => {
                let script = e.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next()).map(|a| self.ex.natural(a)).unwrap_or_default();
                let label = format!("run {script} on another thread (worker)");
                self.simple(s, Concept::Spawn, label, Some(e), vec!["node:worker".into()], cur)
            }
            "new_expression" => {
                let label = format!("create {}", self.ex.natural(e));
                self.simple(s, Concept::Construct, label, Some(e), Vec::new(), cur)
            }
            "sequence_expression" => {
                let mut cur = cur;
                for part in named_children(e) {
                    cur = self.expression_statement(part, part, cur);
                }
                cur
            }
            // `ready && start()` / `cached || load()`: the right side runs only sometimes.
            "binary_expression"
                if e.child_by_field_name("operator").is_some_and(|o| matches!(src.text(o), "&&" | "||" | "??")) =>
            {
                let op = e.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_default();
                let left = e.child_by_field_name("left").map(|l| self.ex.natural(l)).unwrap_or_default();
                let right = e.child_by_field_name("right").map(|r| self.ex.natural(r)).unwrap_or_default();
                let label = match op.as_str() {
                    "&&" => format!("if {left}, {right}"),
                    "||" => format!("unless {left}, {right}"),
                    _ => format!("if {left} is missing, {right}"),
                };
                self.simple(s, Concept::Call, label, Some(e), vec!["short-circuit".into()], cur)
            }
            // `kind satisfies never;`: the compiler checks every case was handled before here.
            "satisfies_expression" if src.text(e).trim_end().ends_with("never") => {
                let value = named_children(e).into_iter().next();
                let label = format!("check while compiling that every case of {} was handled", value.map(|v| self.ex.natural(v)).unwrap_or_default());
                self.simple(s, Concept::TypeCheck, label, Some(e), vec!["exhaustive-check".into()], cur)
            }
            "ternary_expression" => {
                let label = format!("choose: {}", self.ex.natural(e));
                self.simple(s, Concept::Call, label, Some(e), vec!["short-circuit".into()], cur)
            }
            "unary_expression"
                if named_children(e).into_iter().last().map(unparen).is_some_and(|c| {
                    c.kind() == "call_expression" && c.child_by_field_name("function").is_some_and(|f| f.kind() == "parenthesized_expression")
                }) =>
            {
                let call = named_children(e).into_iter().last().map(unparen).unwrap_or(e);
                self.call(s, call, cur)
            }
            "unary_expression" => {
                let label = self.ex.natural(e);
                self.simple(s, Concept::Call, label, Some(e), Vec::new(), cur)
            }
            "arrow_function" | "function_expression" | "function" => {
                // Arrow function expression bodies that are themselves functions, or IIFE-less stray functions.
                let label = format!("create {}", self.ex.natural(e));
                self.simple(s, Concept::Lambda, label, Some(e), Vec::new(), cur)
            }
            "identifier" | "member_expression" | "subscript_expression" | "string" | "number" | "template_string"
            | "true" | "false" | "null" | "undefined" | "this" | "object" | "array" | "as_expression"
            | "satisfies_expression" | "non_null_expression" | "jsx_element" | "jsx_self_closing_element" | "binary_expression" => {
                // An expression whose value is used (arrow bodies) or discarded.
                let label = self.ex.natural(e);
                self.simple(s, Concept::Call, label, Some(e), vec!["value".into()], cur)
            }
            _ => {
                let label = src.norm(e);
                self.simple(s, Concept::Opaque, label, Some(e), vec![format!("syntax:{}", e.kind())], cur)
            }
        }
    }

    /// `using r = open()` / `await using r = openAsync()`: disposed at the end of the block.
    fn using_declaration(&mut self, s: Node, assign: Node, is_async: bool, cur: Flow) -> Flow {
        let src = self.src;
        let left = assign.child_by_field_name("left");
        let value = assign.child_by_field_name("right");
        let name = left.map(|l| src.norm(l)).unwrap_or_default();
        let label = format!("{name} = {}, disposed at the end of the block", value.map(|v| self.ex.natural(v)).unwrap_or_default());
        let mut flavors = vec!["dispose-at-end".to_owned()];
        if is_async {
            flavors.push("async".into());
        }
        let lowered = value.map(|v| self.ex.lower(v));
        let spec = NodeSpec::new(Concept::Var, self.label(label), src.norm(s), src.span(s)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, value);
        if !name.is_empty() {
            self.b.define(&id, &name, None);
        }
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn assignment(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let left = e.child_by_field_name("left");
        let right = e.child_by_field_name("right");
        let op = e.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_else(|| "=".into());
        let left_text = left.map(|l| src.norm(l)).unwrap_or_default();
        let right_text = right.map(|r| self.ex.natural(r)).unwrap_or_default();

        // `button.onclick = () => …`: a handler property is a subscription.
        let prop = left
            .filter(|l| l.kind() == "member_expression")
            .and_then(|l| l.child_by_field_name("property"))
            .map(|p| src.norm(p))
            .unwrap_or_default();
        if op == "=" && prop.starts_with("on") && prop.len() > 2 && prop[2..].starts_with(char::is_lowercase)
            && right.is_some_and(|r| is_function_value(unparen(r).kind()) || unparen(r).kind() == "identifier")
        {
            let label = format!("on {left_text}, run {right_text}");
            return self.simple(s, Concept::Subscribe, label, Some(e), vec!["handler-property".into()], cur);
        }

        if let (Some(l), Some(r)) = (left, right.map(unparen))
            && op == "="
            && is_function_value(r.kind())
            && self.functions.contains_key(&r.start_byte())
        {
            let name = src.norm(l).replace(".prototype.", ".");
            return self.define_function(s, r, &name, cur);
        }

        // `this.subscription = source$.subscribe(v => { … })`.
        if op == "="
            && let Some(r) = right.map(unparen).filter(|r| r.kind() == "call_expression")
            && let Some((label, flavor, target)) = self.registration_value(r)
        {
            let concept = if left.is_some_and(|l| l.kind() == "identifier") { Concept::Assign } else { Concept::SetMember };
            let mut spec = NodeSpec::new(concept, self.label(format!("set {left_text} to: {label}")), src.norm(s), src.span(s))
                .flavor(flavor)
                .expr(Some(self.ex.lower(e)));
            if let Some(t) = target {
                spec = spec.call(t);
            }
            let id = self.place(spec, &cur);
            self.read_all(&id, right);
            if let Some(l) = left.filter(|l| l.kind() == "identifier") {
                self.b.write(&id, &src.norm(l));
            }
            return Some(self.b.exec_out(&id, "out", ""));
        }

        let mut flavors = Vec::new();
        let concept = match left.map(|l| l.kind()) {
            Some("identifier") => Concept::Assign,
            Some("subscript_expression") => {
                flavors.push("index".to_owned());
                Concept::Assign
            }
            Some("object_pattern" | "array_pattern") => Concept::Destructure,
            _ => Concept::SetMember,
        };
        if left_text == "module.exports" || left_text.starts_with("exports.") || left_text.starts_with("module.exports.") {
            flavors.push("commonjs".into());
            flavors.push("export".into());
        }
        if right.is_some_and(|r| unparen(r).kind() == "await_expression") {
            flavors.push("async".into());
        }
        let label = match op.as_str() {
            "=" if flavors.iter().any(|f| f == "commonjs") => format!("export {right_text} as {left_text}"),
            "=" => format!("set {left_text} to {right_text}"),
            "+=" => format!("add {right_text} to {left_text}"),
            "-=" => format!("subtract {right_text} from {left_text}"),
            "*=" => format!("multiply {left_text} by {right_text}"),
            "/=" => format!("divide {left_text} by {right_text}"),
            "**=" => format!("raise {left_text} to the power {right_text}"),
            "??=" => {
                flavors.push("fallback".into());
                format!("if {left_text} is null or undefined, set it to {right_text}")
            }
            "||=" => {
                flavors.push("truthiness".into());
                format!("if {left_text} is falsy, set it to {right_text}")
            }
            "&&=" => {
                flavors.push("truthiness".into());
                format!("if {left_text} is truthy, set it to {right_text}")
            }
            _ => format!("{left_text} {op} {right_text}"),
        };
        let lowered = self.ex.lower(e);
        let spec = NodeSpec::new(concept, self.label(label), src.norm(s), src.span(s)).flavors(flavors).expr(Some(lowered));
        let id = self.place(spec, &cur);
        if op == "=" && left.is_some_and(|l| l.kind() == "identifier") {
            self.read_all(&id, right);
        } else {
            self.read_all(&id, Some(e));
        }
        if let Some(l) = left {
            match l.kind() {
                "identifier" => self.b.write(&id, &src.norm(l)),
                "object_pattern" | "array_pattern" => {
                    let mut names = Vec::new();
                    pattern_names(src, l, &mut names);
                    for n in names {
                        self.b.write(&id, &n);
                    }
                }
                _ => {}
            }
        }
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn call(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let func = e.child_by_field_name("function");
        let args: Vec<Node> = e.child_by_field_name("arguments").map(named_children).unwrap_or_default();
        let (method, target) = match func {
            Some(f) if f.kind() == "member_expression" => (
                f.child_by_field_name("property").map(|p| src.norm(p)).unwrap_or_default(),
                f.child_by_field_name("object"),
            ),
            Some(f) => (src.norm(f), None),
            None => (String::new(), None),
        };
        let target_text = target.map(|t| self.ex.natural(t)).unwrap_or_default();
        let event = args.first().map(|a| if a.kind() == "string" { format!("'{}'", string_value(src, *a)) } else { self.ex.natural(*a) });

        // A callback handed to a framework or an event source.
        if let Some(reg) = frameworks::registration(src, self.fw, e) {
            let callback = self.functions.get(&reg.callback.start_byte()).cloned();
            let run = if callback.is_some() { format!("run {}", reg.name) } else { format!("run {}", self.ex.natural(reg.callback)) };
            let label = reg.statement.replace("run …", &run);
            let mut spec = NodeSpec::new(Concept::Subscribe, self.label(label), src.header(s, Some(reg.callback)), src.span(s))
                .flavor(reg.flavor)
                .expr(Some(self.ex.lower(e)));
            if method == "once" {
                spec = spec.flavor("once");
            }
            if let Some(id) = callback {
                spec = spec.call(CallTarget { symbol: Some(id), display: reg.name.clone(), external: false });
            }
            let id = self.place(spec, &cur);
            for a in args.iter().filter(|a| a.id() != reg.callback.id()) {
                self.read_all(&id, Some(*a));
            }
            return Some(self.b.exec_out(&id, "out", ""));
        }
        // `process.exit(1)`, `readFileSync(…)`, `spawn('git', …)`.
        if let Some((concept, label, flavor)) = frameworks::node_call(src, self.fw, e) {
            return self.simple(s, concept, label, Some(e), vec![flavor.into()], cur);
        }
        // React state changes: `setCount(c => c + 1)`, `this.setState({ … })`.
        let setter = target.is_none() && (self.setters.contains(&method) || self.fw.react && frameworks::is_setter_name(&method));
        if setter || (method == "setState" && target_text == "this") {
            let label = format!("change state with {}{} (React renders again)", if target.is_some() { "this.setState" } else { &method }, src.norm(e.child_by_field_name("arguments").unwrap_or(e)));
            return self.simple(s, Concept::Call, self.label(label), Some(e), vec!["react:set-state".into()], cur);
        }
        // Vue: `emit('save', item)`, `this.$emit('save', item)`.
        if self.fw.vue && (method == "emit" && target.is_none() || method == "$emit") && args.first().is_some_and(|a| a.kind() == "string") {
            let rest: Vec<String> = args.iter().skip(1).map(|a| self.ex.natural(*a)).collect();
            let with = if rest.is_empty() { String::new() } else { format!(" with {}", rest.join(", ")) };
            let label = format!("raise {} to the parent component{with}", event.clone().unwrap_or_default());
            return self.simple(s, Concept::Emit, label, Some(e), vec!["vue:emit".into()], cur);
        }
        // `this.saved.emit(hero)` (Angular outputs, RxJS subjects): the value is the event.
        if target.is_some() && method == "emit" && args.len() == 1 && args[0].kind() != "string" {
            let label = format!("send {} through {target_text}", self.ex.natural(args[0]));
            let flavors = if self.fw.angular { vec!["angular:output".to_owned()] } else { Vec::new() };
            return self.simple(s, Concept::Emit, label, Some(e), flavors, cur);
        }

        if target.is_some() && args.len() >= 2 && SUBSCRIBE.contains(&method.as_str()) {
            let handler = self.ex.natural(args[1]);
            let label = format!("on {target_text} {}, run {handler}", event.unwrap_or_default());
            let flavors = if method == "once" { vec!["once".to_owned()] } else { Vec::new() };
            return self.simple(s, Concept::Subscribe, label, Some(e), flavors, cur);
        }
        if target.is_some() && !args.is_empty() && UNSUBSCRIBE.contains(&method.as_str()) {
            let label = format!("stop listening to {} on {target_text}", event.unwrap_or_default());
            return self.simple(s, Concept::Subscribe, label, Some(e), vec!["unsubscribe".into()], cur);
        }
        if target.is_some() && !args.is_empty() && EMIT.contains(&method.as_str()) {
            let rest: Vec<String> = args.iter().skip(1).map(|a| self.ex.natural(*a)).collect();
            let with = if rest.is_empty() { String::new() } else { format!(" with {}", rest.join(", ")) };
            let label = format!("raise {} on {target_text}{with}", event.unwrap_or_default());
            return self.simple(s, Concept::Emit, label, Some(e), Vec::new(), cur);
        }
        if target.is_none() && matches!(method.as_str(), "setTimeout" | "setInterval" | "setImmediate" | "queueMicrotask" | "requestAnimationFrame") {
            let callback = args.first().map(|a| self.ex.natural(*a)).unwrap_or_default();
            let label = match (method.as_str(), args.get(1)) {
                ("setTimeout", Some(ms)) => format!("after {} ms, run {callback}", src.norm(*ms)),
                ("setInterval", Some(ms)) => format!("every {} ms, run {callback}", src.norm(*ms)),
                ("requestAnimationFrame", _) => format!("on the next frame, run {callback}"),
                _ => format!("soon, run {callback}"),
            };
            let flavors = if method == "setInterval" { vec!["repeating".to_owned()] } else { Vec::new() };
            return self.simple(s, Concept::Spawn, self.label(label), Some(e), flavors, cur);
        }

        // `(async () => { … })()` with its own Blueprint.
        if let Some(callee) = func.filter(|f| f.kind() == "parenthesized_expression").map(unparen)
            && let Some(id) = self.functions.get(&callee.start_byte())
        {
            let spec = NodeSpec::new(Concept::Call, "run the function written here", src.header(s, callee.child_by_field_name("body")), src.span(s))
                .flavor("iife")
                .call(CallTarget { symbol: Some(id.clone()), display: "(immediately run)".into(), external: false });
            let node = self.place(spec, &cur);
            return Some(self.b.exec_out(&node, "out", ""));
        }

        let mut flavors = Vec::new();
        if let Some(p) = self.ex.pipeline(e) {
            flavors.push(match p.kind {
                PipelineKind::Array => "array-pipeline".to_owned(),
                PipelineKind::Promise => "promise".to_owned(),
                PipelineKind::Rx => "rx-pipeline".to_owned(),
            });
        }
        let label = self.ex.natural(e);
        self.simple(s, Concept::Call, label, Some(e), flavors, cur)
    }

    /// A registration used as a value: its statement label, flavor and the callback's Blueprint.
    fn registration_value(&mut self, call: Node) -> Option<(String, &'static str, Option<CallTarget>)> {
        let reg = frameworks::registration(self.src, self.fw, call)?;
        let callback = self.functions.get(&reg.callback.start_byte()).cloned();
        let run = if callback.is_some() { format!("run {}", reg.name) } else { format!("run {}", self.ex.natural(reg.callback)) };
        let target = callback.map(|id| CallTarget { symbol: Some(id), display: reg.name.clone(), external: false });
        Some((reg.statement.replace("run …", &run), reg.flavor, target))
    }

    /// `await sleep(100)`, `await new Promise(r => setTimeout(r, 100))`: a pause.
    fn wait_label(&self, inner: Node) -> Option<String> {
        let src = self.src;
        let inner = unparen(inner);
        match inner.kind() {
            "call_expression" => {
                let f = inner.child_by_field_name("function")?;
                let name = src.norm(f);
                let short = name.rsplit('.').next().unwrap_or(&name);
                if !matches!(short, "sleep" | "delay" | "wait" | "setTimeout" | "timeout") {
                    return None;
                }
                let ms = inner.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next())?;
                Some(wait_text(&src.norm(ms)))
            }
            "new_expression" => {
                let ctor = inner.child_by_field_name("constructor").map(|c| src.norm(c))?;
                let text = src.norm(inner);
                (ctor == "Promise" && text.contains("setTimeout")).then(|| {
                    let ms = text.rsplit(',').next().unwrap_or("").trim_end_matches([')', ' ', ';', '}']).trim();
                    wait_text(ms)
                })
            }
            _ => None,
        }
    }

    fn return_value(&mut self, s: Node, e: Option<Node>, cur: Flow) -> Flow {
        let mut flavors = Vec::new();
        let label = match e {
            Some(e) => {
                let bare = unparen(e);
                if bare.kind() == "await_expression" {
                    flavors.push("async".to_owned());
                }
                if let Some(p) = self.ex.pipeline(bare) {
                    flavors.push(match p.kind {
                        PipelineKind::Array => "array-pipeline".into(),
                        PipelineKind::Promise => "promise".into(),
                        PipelineKind::Rx => "rx-pipeline".into(),
                    });
                }
                if matches!(bare.kind(), "jsx_element" | "jsx_self_closing_element") {
                    flavors.push("jsx".to_owned());
                    if self.component {
                        return self.simple(s, Concept::Return, format!("show {}", self.ex.natural(e)), Some(e), flavors, cur);
                    }
                }
                format!("return {}", self.ex.natural(e))
            }
            None => "return".to_owned(),
        };
        self.simple(s, Concept::Return, label, e, flavors, cur)
    }

    /// Conditions that rely on JavaScript truthiness (`if (user)`, `while (!done)`).
    fn truthiness(&self, cond: Node) -> bool {
        let cond = unparen(cond);
        match cond.kind() {
            "identifier" | "member_expression" | "subscript_expression" => true,
            "unary_expression" => {
                cond.child_by_field_name("operator").is_some_and(|o| self.src.text(o) == "!")
                    && cond.child_by_field_name("argument").is_some_and(|a| {
                        matches!(unparen(a).kind(), "identifier" | "member_expression" | "subscript_expression")
                    })
            }
            _ => false,
        }
    }

    fn if_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let cond = s.child_by_field_name("condition").map(unparen);
        let cons = s.child_by_field_name("consequence");
        let alt = s
            .child_by_field_name("alternative")
            .map(|a| if a.kind() == "else_clause" { named_children(a).into_iter().next().unwrap_or(a) } else { a });
        let label = format!("if {}", cond.map(|c| self.ex.natural(c)).unwrap_or_default());
        let lowered = cond.map(|c| self.ex.lower(c));
        let mut spec = NodeSpec::new(Concept::Branch, self.label(label), src.header(s, cons), src.header_span(s, cons)).expr(lowered);
        if cond.is_some_and(|c| self.truthiness(c)) {
            spec = spec.flavor("truthiness");
        }
        let id = self.place(spec, &cur);
        self.read_all(&id, cond);
        let t = self.b.exec_out(&id, "true", "True");
        let f = self.b.exec_out(&id, "false", "False");
        let t_end = self.body(cons, Some(t));
        let f_end = match alt {
            Some(a) => self.body(Some(a), Some(f)),
            None => Some(f),
        };
        if alt.is_some() && t_end.is_none() && f_end.is_none() {
            return None;
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn switch_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let value = s.child_by_field_name("value").map(unparen);
        let body = s.child_by_field_name("body");
        let label = format!("match {}", value.map(|v| self.ex.natural(v)).unwrap_or_default());
        let lowered = value.map(|v| self.ex.lower(v));
        let spec = NodeSpec::new(Concept::Match, self.label(label), src.header(s, body), src.header_span(s, body)).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, value);
        let Some(body) = body else { return Some(self.b.exec_out(&id, "out", "Then")) };
        // Empty cases fall through to the next one: `case "a": case "b": …` is one path.
        let mut labels: Vec<String> = Vec::new();
        let mut pin_index = 0;
        let cases: Vec<Node> = named_children(body).into_iter().filter(|c| matches!(c.kind(), "switch_case" | "switch_default")).collect();
        let count = cases.len();
        for (i, case) in cases.into_iter().enumerate() {
            let case_label = match case.kind() {
                "switch_case" => case.child_by_field_name("value").map(|v| src.norm(v)).unwrap_or_default(),
                "switch_default" => "default".to_owned(),
                _ => continue,
            };
            labels.push(case_label);
            let mut stmts: Vec<Node> = named_children(case)
                .into_iter()
                .filter(|c| is_statement(*c) && case.child_by_field_name("value").is_none_or(|v| v.id() != c.id()))
                .collect();
            if stmts.is_empty() {
                continue;
            }
            let ends_with_break = stmts.last().is_some_and(|l| l.kind() == "break_statement" && l.child_by_field_name("label").is_none());
            if ends_with_break {
                stmts.pop();
            }
            let pin_label = truncate(&labels.join(" | "), 40);
            labels.clear();
            let pin = self.b.exec_out(&id, &format!("case:{pin_index}"), &pin_label);
            pin_index += 1;
            self.b.push_scope();
            let before = self.b.node_count();
            let end = self.statements(&stmts, Some(pin));
            self.b.pop_scope();
            // A path that neither breaks nor leaves continues into the next case.
            if !ends_with_break && end.is_some() && self.b.node_count() > before && i + 1 < count {
                self.b.add_flavor_at(before, "fallthrough");
            }
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn for_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let init = s.child_by_field_name("initializer").filter(|n| n.kind() != "empty_statement");
        let cond = s.child_by_field_name("condition").filter(|n| n.kind() != "empty_statement").map(|c| {
            if c.kind() == "expression_statement" { named_children(c).into_iter().next().unwrap_or(c) } else { c }
        });
        let update = s.child_by_field_name("increment");
        let body = s.child_by_field_name("body");
        if init.is_none() && cond.is_none() && update.is_none() {
            let spec = NodeSpec::new(Concept::LoopInfinite, "repeat forever", src.header(s, body), src.header_span(s, body));
            let id = self.place(spec, &cur);
            let b = self.b.exec_out(&id, "body", "Loop body");
            self.body(body, Some(b));
            return Some(self.b.exec_out(&id, "out", "Completed"));
        }
        let mut parts = Vec::new();
        if let Some(i) = init {
            parts.push(format!("start {}", src.norm(i).trim_end_matches(';')));
        }
        parts.push(match cond {
            Some(c) => format!("while {}", self.ex.natural(c)),
            None => "forever".into(),
        });
        if let Some(u) = update {
            parts.push(format!("step {}", src.norm(u)));
        }
        let label = format!("repeat: {}", parts.join("; "));
        let lowered = cond.map(|c| self.ex.lower(c));
        let spec = NodeSpec::new(Concept::LoopRange, self.label(label), src.header(s, body), src.header_span(s, body)).expr(lowered);
        let id = self.place(spec, &cur);
        self.b.push_scope();
        if let Some(i) = init {
            if matches!(i.kind(), "lexical_declaration" | "variable_declaration") {
                for d in named_children(i).into_iter().filter(|d| d.kind() == "variable_declarator") {
                    self.read_all(&id, d.child_by_field_name("value"));
                    if let Some(n) = d.child_by_field_name("name") {
                        let mut names = Vec::new();
                        pattern_names(src, n, &mut names);
                        for name in names {
                            self.b.define(&id, &name, None);
                        }
                    }
                }
            } else {
                self.read_all(&id, Some(i));
            }
        }
        self.read_all(&id, cond);
        let b = self.b.exec_out(&id, "body", "Loop body");
        self.body(body, Some(b));
        self.b.pop_scope();
        Some(self.b.exec_out(&id, "out", "Completed"))
    }

    fn for_in_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let left = s.child_by_field_name("left");
        let right = s.child_by_field_name("right");
        let body = s.child_by_field_name("body");
        let tokens: Vec<String> = children(s).into_iter().filter(|c| !c.is_named()).map(|c| src.text(c).to_owned()).collect();
        let is_of = tokens.iter().any(|t| t == "of");
        let mut flavors = Vec::new();
        if tokens.iter().any(|t| t == "await") {
            flavors.push("async".to_owned());
        }
        if tokens.iter().any(|t| t == "var") {
            flavors.push("var".to_owned());
        }
        // `for (using r of rs)` / `for (await using r of rs)`: each item is disposed after its turn.
        let header = src.header(s, body);
        if crate::syntax::has_word(&header, "using") && left.is_some_and(|l| !crate::syntax::has_word(&src.norm(l), "using")) {
            flavors.push("dispose-at-end".to_owned());
            if header.contains("await using") {
                flavors.push("async".to_owned());
            }
        }
        let left_text = left.map(|l| src.norm(l)).unwrap_or_default();
        let right_text = right.map(|r| self.ex.natural(r)).unwrap_or_default();
        let label = if is_of {
            format!("for each {left_text} in {right_text}")
        } else {
            flavors.push("keys".to_owned());
            format!("for each property name {left_text} in {right_text}")
        };
        let lowered = right.map(|r| self.ex.lower(r));
        let spec = NodeSpec::new(Concept::LoopForEach, self.label(label), src.header(s, body), src.header_span(s, body))
            .flavors(flavors)
            .expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, right);
        self.b.push_scope();
        if let Some(l) = left {
            let mut names = Vec::new();
            pattern_names(src, l, &mut names);
            for n in names {
                self.b.define(&id, &n, None);
            }
        }
        let b = self.b.exec_out(&id, "body", "Loop body");
        self.body(body, Some(b));
        self.b.pop_scope();
        Some(self.b.exec_out(&id, "out", "Completed"))
    }

    fn while_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let cond = s.child_by_field_name("condition").map(unparen);
        let body = s.child_by_field_name("body");
        let is_do = s.kind() == "do_statement";
        let cond_text = cond.map(|c| self.ex.natural(c)).unwrap_or_default();
        let (concept, label, mut flavors) = if cond_text == "true" {
            (Concept::LoopInfinite, "repeat forever".to_owned(), Vec::new())
        } else if is_do {
            (Concept::LoopWhile, format!("repeat, then check {cond_text}"), vec!["post-check".to_owned()])
        } else {
            (Concept::LoopWhile, format!("while {cond_text}"), Vec::new())
        };
        if cond.is_some_and(|c| self.truthiness(c)) {
            flavors.push("truthiness".into());
        }
        let (code, span) = if is_do {
            (format!("do … while ({})", cond.map(|c| src.norm(c)).unwrap_or_default()), src.span(s))
        } else {
            (src.header(s, body), src.header_span(s, body))
        };
        let lowered = cond.map(|c| self.ex.lower(c));
        let spec = NodeSpec::new(concept, self.label(label), code, span).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, cond);
        let b = self.b.exec_out(&id, "body", "Loop body");
        self.body(body, Some(b));
        Some(self.b.exec_out(&id, "out", "Completed"))
    }

    fn try_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let body = s.child_by_field_name("body");
        let spec = NodeSpec::new(Concept::Try, "try; handle errors below".to_owned(), "try".to_owned(), src.header_span(s, body));
        let id = self.place(spec, &cur);
        let t = self.b.exec_out(&id, "try", "Try");
        self.body(body, Some(t));
        if let Some(clause) = s.child_by_field_name("handler") {
            let param = clause.child_by_field_name("parameter");
            let ty = clause.child_by_field_name("type").map(|t| crate::syntax::type_text(src, t));
            let label = match param {
                Some(p) => format!("on error {}", src.norm(p)),
                None => "on any error".to_owned(),
            };
            let pin = self.b.exec_out(&id, "catch:0", &truncate(&label, 40));
            self.b.push_scope();
            if let Some(p) = param {
                let mut names = Vec::new();
                pattern_names(src, p, &mut names);
                for n in names {
                    self.b.define(&id, &n, ty.clone());
                }
            }
            self.body(clause.child_by_field_name("body"), Some(pin));
            self.b.pop_scope();
        }
        if let Some(fin) = s.child_by_field_name("finalizer") {
            let pin = self.b.exec_out(&id, "finally", "Finally (always)");
            let fin_body = fin.child_by_field_name("body").or_else(|| first_child_of_kind(fin, "statement_block"));
            self.body(fin_body, Some(pin));
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }
}

/// `wait 100 ms`, `wait delayMs`; never `wait ms ms`.
fn wait_text(ms: &str) -> String {
    if !ms.is_empty() && ms.chars().all(|c| c.is_ascii_digit() || c == '_' || c == '.') {
        format!("wait {ms} ms")
    } else if ms.to_ascii_lowercase().ends_with("ms") {
        format!("wait {ms}")
    } else {
        format!("wait {ms} ms")
    }
}
