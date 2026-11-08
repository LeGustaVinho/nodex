//! Function bodies → Blueprint flow graphs.
//!
//! Conventions shared by every language (see the "Visual Language" doc):
//! - Each statement is one impure node on the exec chain; its expression is kept as an `Expr`
//!   tree that the UI expands on demand.
//! - Compound statements (if, loops, try, using, lock) own named exec outputs for their bodies
//!   (`true`, `body`, `try`, ...). A body chain simply ends; execution then resumes at the
//!   compound node's `out` pin ("Then" / "Completed").
//! - Locals get def→use data edges from the node that declares them.

use nodex_ir::{CallTarget, Concept, FlowGraph, Language, TypeRef, TypeRelation};
use nodex_lang_api::kit::{ExecCursor, FlowBuilder, NodeSpec, truncate};
use tree_sitter::Node;

use crate::expr::{ExprLowering, reads, unparen};
use crate::structure::{BodyJob, BodyKind};
use crate::syntax::{
    Src, bare_type_name, children, children_of_kind, first_child_of_kind, is_inert_directive, local_function_id,
    named_children, preproc_body, preproc_label,
};
use crate::unity;

pub struct Lowered {
    pub graph: FlowGraph,
    pub type_refs: Vec<TypeRef>,
    pub symbol_flavors: Vec<String>,
}

type Flow = Option<ExecCursor>;

struct Lowering<'a> {
    src: Src<'a>,
    /// Owner of local functions declared in this body.
    owner: String,
    b: FlowBuilder,
    ex: ExprLowering<'a>,
    unity: bool,
    coroutine: bool,
    label_max: usize,
}

const ENTRY_FLAVORS: &[&str] = &[
    "async", "static", "iterator", "unity:coroutine", "unity:message", "extension", "override", "virtual",
    "abstract", "getter", "setter", "computed", "top-level", "operator",
];

pub fn lower_body(src: Src, job: &BodyJob, callbacks: &std::collections::HashMap<usize, nodex_ir::SymbolId>) -> Lowered {
    let mut l = Lowering {
        src,
        owner: job.owner.clone(),
        b: FlowBuilder::new(job.symbol.clone()),
        ex: ExprLowering { functions: callbacks.clone(), ..ExprLowering::new(src) },
        unity: job.unity,
        coroutine: job.flavors.iter().any(|f| f == "unity:coroutine"),
        label_max: 96,
    };

    // Entry node.
    let (concept, label) = match (job.unity_message, job.kind) {
        (Some(msg), _) => (Concept::EventEntry, format!("Unity calls {} {}", msg, unity::lifecycle_description(msg))),
        (None, BodyKind::Constructor) => (Concept::Entry, format!("when a new {} is created", job.name)),
        (None, BodyKind::Getter) => (Concept::Entry, format!("when {} is read", job.name.trim_end_matches(".get"))),
        (None, BodyKind::Setter) => (Concept::Entry, format!("when {} is assigned", job.name.trim_end_matches(".set").trim_end_matches(".init"))),
        (None, BodyKind::TopLevel) => (Concept::Entry, "when the program starts".to_owned()),
        (None, BodyKind::EventAdd) => (Concept::Entry, format!("when a handler subscribes to {}", job.name.trim_end_matches(".add"))),
        (None, BodyKind::EventRemove) => (Concept::Entry, format!("when a handler unsubscribes from {}", job.name.trim_end_matches(".remove"))),
        (None, BodyKind::LocalFunction) if job.flavors.iter().any(|f| f == "callback") => {
            let params: Vec<&str> = job.params.iter().map(|(n, _)| n.as_str()).collect();
            (Concept::Entry, nodex_lang_api::kit::callback_entry_label(&job.name, &params))
        }
        (None, BodyKind::LocalFunction) => {
            let params: Vec<&str> = job.params.iter().map(|(n, _)| n.as_str()).collect();
            (Concept::Entry, format!("when local function {}({}) is called", job.name, params.join(", ")))
        }
        (None, BodyKind::Method) => {
            let params: Vec<&str> = job.params.iter().map(|(n, _)| n.as_str()).collect();
            (Concept::Entry, format!("when {}({}) is called", job.name, params.join(", ")))
        }
    };
    let entry_span = src.header_span(job.decl, Some(job.body).filter(|b| b.id() != job.decl.id()));
    let flavors = job.flavors.iter().filter(|f| ENTRY_FLAVORS.contains(&f.as_str()) || f.starts_with('#')).cloned();
    let entry = l.b.add(NodeSpec::new(concept, label, job.signature.clone(), entry_span).flavors(flavors));
    let start = l.b.exec_out(&entry, "out", "");
    for (name, ty) in &job.params {
        if !name.is_empty() {
            l.b.define(&entry, name, ty.clone());
        }
    }

    // Body.
    match job.body.kind() {
        "arrow_expression_clause" => {
            if let Some(expr) = named_children(job.body).into_iter().find(|c| c.kind() != "comment") {
                if job.returns_void || matches!(job.kind, BodyKind::Setter | BodyKind::Constructor) {
                    l.expression_statement(job.body, expr, Some(start));
                } else {
                    l.return_value(job.body, Some(expr), Some(start));
                }
            }
        }
        "compilation_unit" => {
            let stmts: Vec<Node> = children_of_kind(job.body, "global_statement")
                .into_iter()
                .filter_map(|g| named_children(g).into_iter().next())
                .collect();
            l.statements(&stmts, Some(start));
        }
        _ => {
            l.stmt(job.body, Some(start));
        }
    }

    let type_refs = l
        .ex
        .instantiated
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| TypeRef { from: job.symbol.clone(), name: n.clone(), relation: TypeRelation::Instantiates })
        .collect();
    let graph = l.b.finish(job.name.clone(), Language::Csharp, src.span(job.decl));
    Lowered { graph, type_refs, symbol_flavors: Vec::new() }
}

/// Statement node kinds lowered by [`Lowering::stmt`]; everything else in a block is skipped.
fn is_statement(node: Node) -> bool {
    let kind = node.kind();
    kind != "comment"
        && !is_inert_directive(kind)
        && (kind.ends_with("_statement") || matches!(kind, "block" | "ERROR" | "preproc_if"))
}

impl<'a> Lowering<'a> {
    fn label(&self, text: String) -> String {
        truncate(&text, self.label_max)
    }

    /// Adds a node and links it after `cur`. Unreachable statements get a flavor instead.
    fn place(&mut self, mut spec: NodeSpec, cur: &Flow) -> String {
        if cur.is_none() {
            spec = spec.flavor("unreachable");
        }
        let id = self.b.add(spec);
        if let Some(c) = cur {
            self.b.link(c, &id);
        }
        id
    }

    /// Wires the locals `node` reads, then defines the variables it declares inline
    /// (`out var x`, `is Order o`, `case int n`), which stay in scope after the statement.
    fn read_all(&mut self, id: &String, node: Option<Node>) {
        if let Some(n) = node {
            for name in reads(self.src, n) {
                self.b.read(id, &name);
            }
            for (name, ty) in inline_declarations(self.src, n) {
                self.b.define(id, &name, ty);
            }
        }
    }

    /// Creates a simple statement node; returns its `out` cursor unless the concept is terminal.
    fn simple(&mut self, s: Node, concept: Concept, label: String, expr: Option<Node>, flavors: Vec<String>, cur: Flow) -> Flow {
        let lowered = expr.map(|e| self.ex.lower(e));
        let spec = NodeSpec::new(concept, self.label(label), self.src.norm(s), self.src.span(s))
            .flavors(flavors)
            .expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, expr);
        if concept.is_terminal() {
            None
        } else {
            Some(self.b.exec_out(&id, "out", ""))
        }
    }

    fn statements(&mut self, stmts: &[Node], mut cur: Flow) -> Flow {
        for s in stmts {
            cur = self.stmt(*s, cur);
        }
        cur
    }

    /// Lowers a statement (or block) as a nested body in its own variable scope.
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
            "block" => {
                let stmts: Vec<Node> = named_children(s).into_iter().filter(|c| is_statement(*c)).collect();
                self.b.push_scope();
                let end = self.statements(&stmts, cur);
                self.b.pop_scope();
                end
            }
            "empty_statement" | "comment" => cur,
            "global_statement" => match named_children(s).into_iter().rfind(|c| is_statement(*c)) {
                Some(inner) => self.stmt(inner, cur),
                None => cur,
            },
            "labeled_statement" => {
                // `retry: x++;` — the label marks a jump target; the statement keeps its own node.
                let label = named_children(s).into_iter().find(|c| c.kind() == "identifier").map(|l| src.norm(l));
                let Some(inner) = named_children(s).into_iter().rfind(|c| is_statement(*c)) else { return cur };
                let before = self.b.node_count();
                let end = self.stmt(inner, cur);
                if let Some(l) = label {
                    self.b.add_flavor_at(before, &format!("label:{l}"));
                }
                end
            }
            "preproc_if" => self.build_switch(s, cur),
            "local_declaration_statement" => self.local_declaration(s, cur),
            "expression_statement" => match named_children(s).into_iter().find(|c| c.kind() != "comment") {
                Some(e) => self.expression_statement(s, e, cur),
                None => cur,
            },
            "if_statement" => self.if_statement(s, cur),
            "switch_statement" => self.switch_statement(s, cur),
            "for_statement" => self.for_statement(s, cur),
            "foreach_statement" => self.foreach_statement(s, cur),
            "while_statement" | "do_statement" => self.while_statement(s, cur),
            "return_statement" => {
                let e = named_children(s).into_iter().find(|c| c.kind() != "comment");
                self.return_value(s, e, cur)
            }
            "throw_statement" => {
                let e = named_children(s).into_iter().find(|c| c.kind() != "comment");
                let label = match e {
                    Some(e) => format!("fail with {}", self.ex.natural(e)),
                    None => "rethrow the current error".into(),
                };
                self.simple(s, Concept::Throw, label, e, Vec::new(), cur)
            }
            "break_statement" => self.simple(s, Concept::Break, "leave the loop".into(), None, Vec::new(), cur),
            "continue_statement" => self.simple(s, Concept::Continue, "skip to the next iteration".into(), None, Vec::new(), cur),
            "goto_statement" => {
                let target = named_children(s).into_iter().next().map(|t| src.norm(t));
                let is_case = children(s).iter().any(|c| src.text(*c) == "case");
                let label = match (target, is_case) {
                    (Some(t), true) => format!("jump to case {t}"),
                    (Some(t), false) => format!("jump to {t}"),
                    (None, _) => "jump to the default case".to_owned(),
                };
                self.simple(s, Concept::Jump, label, None, Vec::new(), cur)
            }
            "yield_statement" => self.yield_statement(s, cur),
            "try_statement" => self.try_statement(s, cur),
            "using_statement" => self.using_statement(s, cur),
            "lock_statement" => {
                let parts = named_children(s);
                let body = parts.iter().rev().find(|c| is_statement(**c)).copied();
                let target = parts.iter().find(|c| !is_statement(**c) && c.kind() != "comment").copied();
                let label = format!("lock {}: one thread at a time", target.map(|t| src.norm(t)).unwrap_or_default());
                self.scoped(s, Concept::CriticalSection, label, target, body, Vec::new(), cur)
            }
            "checked_statement" | "unsafe_statement" | "fixed_statement" => {
                let body = named_children(s).into_iter().rfind(|c| is_statement(*c));
                let flavor = match s.kind() {
                    "checked_statement" if children(s).iter().any(|c| src.text(*c) == "unchecked") => "unchecked".to_owned(),
                    kind => kind.trim_end_matches("_statement").to_owned(),
                };
                let label = match flavor.as_str() {
                    "checked" => "checked region: arithmetic overflow fails".to_owned(),
                    "unchecked" => "unchecked region: arithmetic overflow wraps around".to_owned(),
                    "fixed" => {
                        let decl = first_child_of_kind(s, "variable_declaration").map(|d| src.norm(d)).unwrap_or_default();
                        format!("pin {decl} in memory")
                    }
                    other => format!("{other} region"),
                };
                self.scoped(s, Concept::Scope, label, None, body, vec![flavor], cur)
            }
            "local_function_statement" => {
                let name = s.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let params = s.child_by_field_name("parameters").map(|p| src.norm(p)).unwrap_or_default();
                let body = s.child_by_field_name("body");
                let target = CallTarget {
                    symbol: Some(local_function_id(src, &self.owner, s)),
                    display: format!("{name}{params}"),
                    external: false,
                };
                let spec = NodeSpec::new(
                    Concept::Lambda,
                    format!("define local function {name}{params}"),
                    src.header(s, body),
                    src.header_span(s, body),
                )
                .flavor("local-function")
                .call(target);
                let id = self.place(spec, &cur);
                Some(self.b.exec_out(&id, "out", ""))
            }
            _ => {
                let label = src.norm(s).to_string();
                self.simple(s, Concept::Opaque, label, None, vec![format!("syntax:{}", s.kind())], cur)
            }
        }
    }

    fn local_declaration(&mut self, s: Node, mut cur: Flow) -> Flow {
        let src = self.src;
        let Some(decl) = first_child_of_kind(s, "variable_declaration") else {
            return self.simple(s, Concept::Opaque, src.norm(s), None, Vec::new(), cur);
        };
        let ty = decl.child_by_field_name("type").map(|t| src.norm(t));
        let declared_ty = ty.filter(|t| t != "var");
        let mut flavors = Vec::new();
        for c in children(s) {
            match src.text(c) {
                "using" => flavors.push("dispose-at-end".to_owned()),
                "const" => flavors.push("const".to_owned()),
                "await" => flavors.push("async".to_owned()),
                _ => {}
            }
        }
        if decl.child_by_field_name("type").is_some_and(|t| t.kind() == "ref_type") {
            flavors.push("ref".to_owned());
        }
        let declarators = children_of_kind(decl, "variable_declarator");
        let single = declarators.len() == 1;
        for d in declarators {
            // `var (a, b) = pair;` declares several variables at once.
            if let Some(pattern) = first_child_of_kind(d, "tuple_pattern") {
                let value = named_children(d).into_iter().find(|c| c.id() != pattern.id());
                let mut names = Vec::new();
                collect_identifiers(src, pattern, &mut names);
                let label = format!("unpack {} from {}", src.norm(pattern), value.map(|v| self.ex.natural(v)).unwrap_or_default());
                let lowered = value.map(|v| self.ex.lower(v));
                let concept = if value.is_some_and(|v| unparen(v).kind() == "await_expression") { Concept::Await } else { Concept::Destructure };
                let spec = NodeSpec::new(concept, self.label(label), src.norm(s), src.span(s)).flavors(flavors.clone()).expr(lowered);
                let id = self.place(spec, &cur);
                self.read_all(&id, value);
                for n in names.into_iter().filter(|n| n != "_") {
                    self.b.define(&id, &n, None);
                }
                cur = Some(self.b.exec_out(&id, "out", ""));
                continue;
            }
            let Some(name_node) = d.child_by_field_name("name") else {
                // Never drop code: unknown declarator shapes stay visible.
                cur = self.simple(d, Concept::Opaque, src.norm(d), None, vec!["syntax:variable_declarator".into()], cur);
                continue;
            };
            let name = src.norm(name_node);
            let value = named_children(d)
                .into_iter()
                .filter(|c| c.id() != name_node.id() && c.kind() != "bracketed_argument_list")
                .map(|c| if c.kind() == "equals_value_clause" { named_children(c).into_iter().next().unwrap_or(c) } else { c })
                .next();
            let (concept, label) = match value.map(unparen) {
                Some(v) if v.kind() == "await_expression" => {
                    let inner = named_children(v).into_iter().next().unwrap_or(v);
                    (Concept::Await, format!("{name} = wait for {}", self.ex.natural(inner)))
                }
                Some(v) => (Concept::Var, format!("{name} = {}", self.ex.natural(v))),
                None => (Concept::Var, format!("declare {name}")),
            };
            let code_node = if single { s } else { d };
            let lowered = value.map(|v| self.ex.lower(v));
            let spec = NodeSpec::new(concept, self.label(label), src.norm(code_node), src.span(code_node))
                .flavors(flavors.clone())
                .expr(lowered);
            let id = self.place(spec, &cur);
            self.read_all(&id, value);
            self.b.define(&id, &name, declared_ty.clone());
            cur = Some(self.b.exec_out(&id, "out", ""));
        }
        cur
    }

    fn expression_statement(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let e = unparen(e);
        match e.kind() {
            "assignment_expression" => self.assignment(s, e, cur),
            "postfix_unary_expression" | "prefix_unary_expression" => {
                let op = children(e).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
                let target = named_children(e).into_iter().next();
                let target_text = target.map(|t| src.norm(t)).unwrap_or_default();
                let label = match op.as_str() {
                    "++" => format!("increase {target_text} by 1"),
                    "--" => format!("decrease {target_text} by 1"),
                    _ => src.norm(e),
                };
                let concept = if target.is_some_and(|t| t.kind() == "identifier") { Concept::Assign } else { Concept::SetMember };
                let flow = self.simple(s, concept, label, Some(e), Vec::new(), cur);
                if let (Some(t), Some((id, _))) = (target, &flow)
                    && t.kind() == "identifier" {
                        let id = id.clone();
                        self.b.write(&id, &src.norm(t));
                    }
                flow
            }
            "invocation_expression" => self.invocation(s, e, cur),
            "await_expression" => {
                let inner = named_children(e).into_iter().next().unwrap_or(e);
                let label = format!("wait for {}", self.ex.natural(inner));
                self.simple(s, Concept::Await, label, Some(e), Vec::new(), cur)
            }
            "object_creation_expression" => {
                let label = format!("create {}", self.ex.natural(e));
                self.simple(s, Concept::Construct, label, Some(e), Vec::new(), cur)
            }
            // `void M() => throw new NotImplementedException();`
            "throw_expression" => {
                let inner = named_children(e).into_iter().next();
                let label = format!("fail with {}", inner.map(|i| self.ex.natural(i)).unwrap_or_default());
                self.simple(s, Concept::Throw, label, inner, Vec::new(), cur)
            }
            _ => {
                let label = src.norm(e);
                self.simple(s, Concept::Opaque, label, Some(e), vec![format!("syntax:{}", e.kind())], cur)
            }
        }
    }

    fn assignment(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let left = e.child_by_field_name("left");
        let right = e.child_by_field_name("right");
        let op = e.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_else(|| "=".into());
        let left_text = left.map(|l| src.norm(l)).unwrap_or_default();
        let right_text = right.map(|r| self.ex.natural(r)).unwrap_or_default();
        let left_is_local = left.is_some_and(|l| l.kind() == "identifier" && self.b.lookup(src.text(l)).is_some());

        // Event subscription heuristic: `Thing.Event += handler` where the handler is a lambda
        // or a method group and the target is not a local.
        let last_segment = left_text.rsplit('.').next().unwrap_or("");
        let handler_like = right.is_some_and(|r| {
            matches!(r.kind(), "lambda_expression" | "anonymous_method_expression")
                || (matches!(r.kind(), "identifier" | "member_access_expression")
                    && src.norm(r).rsplit('.').next().is_some_and(|seg| seg.starts_with(char::is_uppercase)))
        });
        if (op == "+=" || op == "-=") && !left_is_local && last_segment.starts_with(char::is_uppercase) && handler_like {
            let (label, flavors) = if op == "+=" {
                (format!("on {left_text}, run {right_text}"), Vec::new())
            } else {
                (format!("stop running {right_text} on {left_text}"), vec!["unsubscribe".to_owned()])
            };
            return self.simple(s, Concept::Subscribe, label, Some(e), flavors, cur);
        }

        let (concept, mut flavors) = match left.map(|l| l.kind()) {
            Some("identifier") if left_is_local => (Concept::Assign, Vec::new()),
            Some("identifier") => (Concept::SetMember, vec!["implicit-this".to_owned()]),
            Some("element_access_expression") => (Concept::Assign, vec!["index".to_owned()]),
            Some("tuple_expression") => (Concept::Destructure, Vec::new()),
            _ => (Concept::SetMember, Vec::new()),
        };
        if right.is_some_and(|r| unparen(r).kind() == "await_expression") {
            flavors.push("async".into());
        }
        // C# 14 `a?.B = c`: nothing happens when `a` is null.
        if left_text.contains("?.") || left_text.contains("?[") {
            flavors.push("safe-access".into());
        }
        let label = match op.as_str() {
            "=" if left_text.contains("?.") => {
                let receiver = left_text.split("?.").next().unwrap_or_default();
                format!("if {receiver} exists, set {left_text} to {right_text}")
            }
            "=" => format!("set {left_text} to {right_text}"),
            "+=" => format!("add {right_text} to {left_text}"),
            "-=" => format!("subtract {right_text} from {left_text}"),
            "*=" => format!("multiply {left_text} by {right_text}"),
            "/=" => format!("divide {left_text} by {right_text}"),
            "??=" => format!("if {left_text} is null, set it to {right_text}"),
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
        if let Some(l) = left.filter(|l| l.kind() == "identifier") {
            self.b.write(&id, &src.norm(l));
        }
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn invocation(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let func = e.child_by_field_name("function");
        let args_text = e.child_by_field_name("arguments").map(|a| src.norm(a)).unwrap_or_else(|| "()".into());
        let mut flavors = Vec::new();

        // Event raise: `Handler?.Invoke(x)` or `Handler.Invoke(x)`.
        if let Some(f) = func {
            let (target, safe) = match f.kind() {
                "conditional_access_expression" => (f.child_by_field_name("condition"), true),
                "member_access_expression" => (f.child_by_field_name("expression"), false),
                _ => (None, false),
            };
            let name = match f.kind() {
                "conditional_access_expression" => first_child_of_kind(f, "member_binding_expression")
                    .and_then(|m| m.child_by_field_name("name"))
                    .map(|n| src.norm(n)),
                "member_access_expression" => f.child_by_field_name("name").map(|n| src.norm(n)),
                _ => None,
            };
            if safe {
                flavors.push("safe-access".to_owned());
            }
            if name.as_deref() == Some("Invoke")
                && let Some(t) = target {
                    let label = format!("raise {}{}", src.norm(t), args_text);
                    return self.simple(s, Concept::Emit, label, Some(e), flavors, cur);
                }
        }

        let callee = func.map(|f| src.norm(f)).unwrap_or_default();
        let short = bare_type_name(callee.rsplit('.').next().unwrap_or(&callee));
        if self.unity && short == "StartCoroutine" {
            let label = format!("start coroutine {}", args_text.trim_matches(['(', ')']));
            return self.simple(s, Concept::Spawn, label, Some(e), vec!["unity:coroutine".into()], cur);
        }
        if matches!(callee.as_str(), "Task.Run" | "Task.Factory.StartNew" | "ThreadPool.QueueUserWorkItem") {
            let label = format!("run {} in the background", truncate(args_text.trim_matches(['(', ')']), 60));
            return self.simple(s, Concept::Spawn, label, Some(e), flavors, cur);
        }
        if self.unity {
            match short.as_str() {
                "GetComponent" | "GetComponentInChildren" | "GetComponentInParent" | "FindObjectOfType" | "FindFirstObjectByType" => {
                    flavors.push("unity:component-lookup".into())
                }
                "SendMessage" | "BroadcastMessage" | "SendMessageUpwards" | "Invoke" | "InvokeRepeating" => {
                    flavors.push("unity:dynamic-message".into())
                }
                _ => {}
            }
        }
        if self.ex.pipeline(e).is_some() {
            flavors.push("linq".into());
        }
        let label = self.ex.natural(e);
        self.simple(s, Concept::Call, label, Some(e), flavors, cur)
    }

    fn return_value(&mut self, s: Node, e: Option<Node>, cur: Flow) -> Flow {
        let mut flavors = Vec::new();
        let label = match e {
            Some(e) => {
                if unparen(e).kind() == "await_expression" {
                    flavors.push("async".to_owned());
                }
                if self.ex.pipeline(unparen(e)).is_some() {
                    flavors.push("linq".to_owned());
                }
                format!("return {}", self.ex.natural(e))
            }
            None => "return".to_owned(),
        };
        self.simple(s, Concept::Return, label, e, flavors, cur)
    }

    fn if_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let cond = s.child_by_field_name("condition");
        let cons = s.child_by_field_name("consequence");
        let alt = s
            .child_by_field_name("alternative")
            .map(|a| if a.kind() == "else_clause" { named_children(a).into_iter().next().unwrap_or(a) } else { a });
        let label = format!("if {}", cond.map(|c| self.ex.natural(c)).unwrap_or_default());
        let lowered = cond.map(|c| self.ex.lower(c));
        let spec = NodeSpec::new(Concept::Branch, self.label(label), src.header(s, cons), src.header_span(s, cons)).expr(lowered);
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
        let value = s.child_by_field_name("value").or_else(|| {
            named_children(s).into_iter().find(|c| !matches!(c.kind(), "switch_body" | "comment"))
        });
        let body = s.child_by_field_name("body").or_else(|| first_child_of_kind(s, "switch_body"));
        let label = format!("match {}", value.map(|v| self.ex.natural(v)).unwrap_or_default());
        let lowered = value.map(|v| self.ex.lower(v));
        let spec = NodeSpec::new(Concept::Match, self.label(label), src.header(s, body), src.header_span(s, body)).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, value);
        if let Some(body) = body {
            for (i, section) in children_of_kind(body, "switch_section").into_iter().enumerate() {
                let mut stmts: Vec<Node> = named_children(section).into_iter().filter(|c| is_statement(*c)).collect();
                let pin_label = case_label(src.header(section, stmts.first().copied()).as_str());
                // `break` ends a switch section; it is not a loop exit.
                if stmts.last().is_some_and(|l| l.kind() == "break_statement") {
                    stmts.pop();
                }
                let pin = self.b.exec_out(&id, &format!("case:{i}"), &truncate(&pin_label, 40));
                self.b.push_scope();
                self.statements(&stmts, Some(pin));
                self.b.pop_scope();
            }
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn for_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let init = s.child_by_field_name("initializer");
        let cond = s.child_by_field_name("condition");
        let update = s.child_by_field_name("update");
        let body = s.child_by_field_name("body");
        let mut parts = Vec::new();
        if let Some(i) = init {
            parts.push(format!("start {}", src.norm(i)));
        }
        parts.push(match cond {
            Some(c) => format!("while {}", self.ex.natural(c)),
            None => "forever".into(),
        });
        // `for (;;)` is an endless loop.
        if init.is_none() && cond.is_none() && update.is_none() {
            let spec = NodeSpec::new(Concept::LoopInfinite, "repeat forever", src.header(s, body), src.header_span(s, body));
            let id = self.place(spec, &cur);
            let b = self.b.exec_out(&id, "body", "Loop body");
            self.body(body, Some(b));
            return Some(self.b.exec_out(&id, "out", "Completed"));
        }
        if let Some(u) = update {
            parts.push(format!("step {}", src.norm(u)));
        }
        let label = format!("repeat: {}", parts.join("; "));
        let lowered = cond.map(|c| self.ex.lower(c));
        let spec = NodeSpec::new(Concept::LoopRange, self.label(label), src.header(s, body), src.header_span(s, body)).expr(lowered);
        let id = self.place(spec, &cur);
        self.b.push_scope();
        if let Some(i) = init {
            if i.kind() == "variable_declaration" {
                let ty = i.child_by_field_name("type").map(|t| src.norm(t)).filter(|t| t != "var");
                for d in children_of_kind(i, "variable_declarator") {
                    for v in named_children(d).into_iter().skip(1) {
                        self.read_all(&id, Some(v));
                    }
                    if let Some(n) = d.child_by_field_name("name") {
                        self.b.define(&id, &src.norm(n), ty.clone());
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

    fn foreach_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let left = s.child_by_field_name("left");
        let right = s.child_by_field_name("right");
        let body = s.child_by_field_name("body");
        let ty = s.child_by_field_name("type").map(|t| src.norm(t)).filter(|t| t != "var");
        let left_text = left.map(|l| src.norm(l)).unwrap_or_default();
        let label = format!("for each {left_text} in {}", right.map(|r| self.ex.natural(r)).unwrap_or_default());
        let mut flavors = Vec::new();
        if children(s).iter().any(|c| src.text(*c) == "await") {
            flavors.push("async".to_owned());
        }
        let lowered = right.map(|r| self.ex.lower(r));
        let spec = NodeSpec::new(Concept::LoopForEach, self.label(label), src.header(s, body), src.header_span(s, body))
            .flavors(flavors)
            .expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, right);
        self.b.push_scope();
        if let Some(l) = left {
            let mut names = Vec::new();
            collect_identifiers(src, l, &mut names);
            let single = names.len() == 1;
            for n in names {
                self.b.define(&id, &n, if single { ty.clone() } else { None });
            }
        }
        let b = self.b.exec_out(&id, "body", "Loop body");
        self.body(body, Some(b));
        self.b.pop_scope();
        Some(self.b.exec_out(&id, "out", "Completed"))
    }

    fn while_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let cond = s.child_by_field_name("condition");
        let body = s.child_by_field_name("body");
        let is_do = s.kind() == "do_statement";
        let cond_text = cond.map(|c| self.ex.natural(c)).unwrap_or_default();
        let (concept, label, flavors) = if cond_text == "true" {
            (Concept::LoopInfinite, "repeat forever".to_owned(), Vec::new())
        } else if is_do {
            (Concept::LoopWhile, format!("repeat, then check {cond_text}"), vec!["post-check".to_owned()])
        } else {
            (Concept::LoopWhile, format!("while {cond_text}"), Vec::new())
        };
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

    fn yield_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        if children(s).iter().any(|c| !c.is_named() && src.text(*c) == "break") {
            return self.simple(s, Concept::Return, "stop producing values".into(), None, vec!["yield-break".into()], cur);
        }
        let value = named_children(s).into_iter().find(|c| c.kind() != "comment");
        if (self.coroutine || self.unity)
            && let Some(v) = value.map(unparen) {
                let ty = (v.kind() == "object_creation_expression")
                    .then(|| v.child_by_field_name("type").map(|t| bare_type_name(src.text(t))))
                    .flatten();
                let args = v
                    .child_by_field_name("arguments")
                    .map(|a| src.norm(a).trim_start_matches('(').trim_end_matches(')').to_owned())
                    .unwrap_or_default();
                if let Some(label) = unity::describe_wait(ty.as_deref(), &args, v.kind() == "null_literal") {
                    return self.simple(s, Concept::Wait, label, Some(v), vec!["unity:coroutine".into()], cur);
                }
            }
        let label = format!("produce {}", value.map(|v| self.ex.natural(v)).unwrap_or_default());
        self.simple(s, Concept::Yield, label, value, Vec::new(), cur)
    }

    fn try_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let body = s.child_by_field_name("body");
        let spec = NodeSpec::new(Concept::Try, "try; handle errors below".to_owned(), "try".to_owned(), src.header_span(s, body));
        let id = self.place(spec, &cur);
        let t = self.b.exec_out(&id, "try", "Try");
        self.body(body, Some(t));

        let mut labels = Vec::new();
        for (i, clause) in children_of_kind(s, "catch_clause").into_iter().enumerate() {
            let decl = first_child_of_kind(clause, "catch_declaration");
            let filter = first_child_of_kind(clause, "catch_filter_clause");
            let ty = decl.and_then(|d| d.child_by_field_name("type")).map(|t| src.norm(t));
            let var = decl.and_then(|d| d.child_by_field_name("name")).map(|n| src.norm(n));
            let mut label = match &ty {
                Some(t) => format!("on {t}"),
                None => "on any error".to_owned(),
            };
            if let Some(f) = filter {
                label.push_str(&format!(" {}", src.norm(f)));
            }
            labels.push(label.clone());
            let pin = self.b.exec_out(&id, &format!("catch:{i}"), &truncate(&label, 40));
            self.b.set_pin_type(&id, &format!("catch:{i}"), ty.as_deref().unwrap_or("*"));
            let catch_body = clause.child_by_field_name("body").or_else(|| named_children(clause).into_iter().rfind(|c| c.kind() == "block"));
            self.b.push_scope();
            if let Some(v) = var {
                self.b.define(&id, &v, ty.clone());
            }
            self.body(catch_body, Some(pin));
            self.b.pop_scope();
        }
        if let Some(fin) = first_child_of_kind(s, "finally_clause") {
            let pin = self.b.exec_out(&id, "finally", "Finally (always)");
            let fin_body = named_children(fin).into_iter().find(|c| c.kind() == "block");
            self.body(fin_body, Some(pin));
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn using_statement(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let body = s.child_by_field_name("body");
        let resource = named_children(s)
            .into_iter()
            .find(|c| body.is_none_or(|b| b.id() != c.id()) && c.kind() != "comment");
        let res_text = resource.map(|r| src.norm(r)).unwrap_or_default();
        let label = format!("use {res_text}, then dispose it");
        let mut flavors = vec!["dispose".to_owned()];
        if children(s).iter().any(|c| src.text(*c) == "await") {
            flavors.push("async".into());
        }
        let lowered = resource.map(|r| self.ex.lower(r));
        let spec = NodeSpec::new(Concept::Scope, self.label(label), src.header(s, body), src.header_span(s, body))
            .flavors(flavors)
            .expr(lowered);
        let id = self.place(spec, &cur);
        self.b.push_scope();
        if let Some(r) = resource {
            if r.kind() == "variable_declaration" {
                let ty = r.child_by_field_name("type").map(|t| src.norm(t)).filter(|t| t != "var");
                for d in children_of_kind(r, "variable_declarator") {
                    for v in named_children(d).into_iter().skip(1) {
                        self.read_all(&id, Some(v));
                    }
                    if let Some(n) = d.child_by_field_name("name") {
                        self.b.define(&id, &src.norm(n), ty.clone());
                    }
                }
            } else {
                self.read_all(&id, Some(r));
            }
        }
        let b = self.b.exec_out(&id, "body", "Body");
        self.body(body, Some(b));
        self.b.pop_scope();
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    #[allow(clippy::too_many_arguments)]
    fn scoped(&mut self, s: Node, concept: Concept, label: String, target: Option<Node>, body: Option<Node>, flavors: Vec<String>, cur: Flow) -> Flow {
        let src = self.src;
        let lowered = target.map(|t| self.ex.lower(t));
        let spec = NodeSpec::new(concept, self.label(label), src.header(s, body), src.header_span(s, body))
            .flavors(flavors)
            .expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, target);
        let b = self.b.exec_out(&id, "body", "Body");
        self.body(body, Some(b));
        Some(self.b.exec_out(&id, "out", "Then"))
    }
}

impl Lowering<'_> {
    /// `#if A … #elif B … #else … #endif` inside a body: one exec path per build variant.
    fn build_switch(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let mut branches = Vec::new();
        let mut node = Some(s);
        while let Some(n) = node {
            branches.push((preproc_label(src, n), preproc_body(n)));
            node = n.child_by_field_name("alternative");
        }
        let labels: Vec<String> = branches.iter().map(|(l, _)| l.clone()).collect();
        let label = format!("build variant: {}", labels.join(" / "));
        let first_body = branches.first().and_then(|(_, b)| b.first().copied());
        let spec = NodeSpec::new(Concept::BuildSwitch, self.label(label), labels[0].clone(), src.header_span(s, first_body))
            .flavor("compile-time");
        let id = self.place(spec, &cur);
        for (i, (label, body)) in branches.into_iter().enumerate() {
            let pin = self.b.exec_out(&id, &format!("variant:{i}"), &label);
            let stmts: Vec<Node> = body.into_iter().filter(|c| is_statement(*c)).collect();
            self.b.push_scope();
            self.statements(&stmts, Some(pin));
            self.b.pop_scope();
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }
}

/// Variables declared inside an expression: `out var x`, `is Order o`, `case int n`,
/// `{ Length: > 3 } s`. Lambdas are skipped (their parameters are their own).
fn inline_declarations(src: Src, node: Node) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "lambda_expression" | "anonymous_method_expression" | "local_function_statement" => continue,
            "declaration_expression" | "declaration_pattern" | "recursive_pattern" | "var_pattern" => {
                let ty = n.child_by_field_name("type").map(|t| src.norm(t)).filter(|t| t != "var");
                let name = n
                    .child_by_field_name("name")
                    .or_else(|| n.child_by_field_name("designation"))
                    .or_else(|| named_children(n).into_iter().rfind(|c| matches!(c.kind(), "identifier" | "single_variable_designation")));
                if let Some(name) = name {
                    let text = src.norm(name);
                    if text != "_" && text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        out.push((text, ty));
                    }
                }
            }
            _ => {}
        }
        stack.extend(named_children(n));
    }
    out.reverse();
    out
}

/// `case OrderStatus.Paid:` → `OrderStatus.Paid`; `default:` → `default`.
fn case_label(header: &str) -> String {
    header
        .split(':')
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.trim().trim_start_matches("case ").trim())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn collect_identifiers(src: Src, node: Node, out: &mut Vec<String>) {
    if node.kind() == "identifier" {
        out.push(src.norm(node));
        return;
    }
    for c in named_children(node) {
        if c.kind().ends_with("type") {
            continue;
        }
        collect_identifiers(src, c, out);
    }
}
