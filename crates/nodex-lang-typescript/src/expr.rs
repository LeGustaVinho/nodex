//! Expressions → [`Expr`] trees, natural-language text and local-variable reads.

use nodex_ir::{Concept, Expr};
use nodex_lang_api::kit::truncate;
use tree_sitter::Node;

use crate::syntax::{Src, bare_type_name, children, is_function_value, named_children, pattern_names, string_value, uses_this};

/// Array methods. A chain with two or more of these is shown as a `Pipeline`, like LINQ.
const ARRAY_OPS: &[&str] = &[
    "filter", "map", "flatMap", "reduce", "reduceRight", "some", "every", "find", "findIndex", "findLast",
    "findLastIndex", "forEach", "sort", "toSorted", "reverse", "toReversed", "slice", "concat", "includes",
    "indexOf", "lastIndexOf", "join", "flat", "at", "entries", "keys", "values", "toSpliced", "with",
];

/// Array methods that always take an argument (without one, the name is more likely a project method).
const ARRAY_OPS_NEED_ARGS: &[&str] = &[
    "filter", "map", "flatMap", "reduce", "reduceRight", "some", "every", "find", "findIndex", "findLast",
    "findLastIndex", "forEach", "includes", "indexOf", "lastIndexOf", "at",
];

/// Promise continuation methods.
const PROMISE_OPS: &[&str] = &["then", "catch", "finally"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineKind {
    /// `xs.filter(…).map(…)`.
    Array,
    /// `fetch(u).then(…).catch(…)`.
    Promise,
}

/// A method-chain pipeline.
pub struct Pipeline<'t> {
    pub kind: PipelineKind,
    pub source: Node<'t>,
    /// `(method name, call node)` from first to last.
    pub stages: Vec<(String, Node<'t>)>,
}

fn has_args(call: Node) -> bool {
    call.child_by_field_name("arguments").is_some_and(|a| !named_children(a).is_empty())
}

fn is_array_stage(name: &str, call: Node) -> bool {
    ARRAY_OPS.contains(&name) && (!ARRAY_OPS_NEED_ARGS.contains(&name) || has_args(call))
}

/// Removes redundant parentheses.
pub fn unparen(mut node: Node) -> Node {
    while node.kind() == "parenthesized_expression" {
        match named_children(node).into_iter().next() {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

/// Lowers expressions and remembers which types were instantiated.
pub struct ExprLowering<'a> {
    pub src: Src<'a>,
    pub instantiated: Vec<String>,
}

impl<'a> ExprLowering<'a> {
    pub fn new(src: Src<'a>) -> Self {
        Self { src, instantiated: Vec::new() }
    }

    /// Detects an array or promise pipeline rooted at `node`.
    pub fn pipeline<'t>(&self, node: Node<'t>) -> Option<Pipeline<'t>> {
        let mut stages = Vec::new();
        let mut cur = unparen(node);
        while cur.kind() == "call_expression" {
            let Some(func) = cur.child_by_field_name("function") else { break };
            if func.kind() != "member_expression" {
                break;
            }
            let Some(prop) = func.child_by_field_name("property") else { break };
            stages.push((self.src.norm(prop), cur));
            let Some(object) = func.child_by_field_name("object") else { break };
            cur = unparen(object);
        }
        stages.reverse();
        if let Some(first) = stages.iter().position(|(n, call)| is_array_stage(n, *call)) {
            let count = stages[first..].iter().filter(|(n, call)| is_array_stage(n, *call)).count();
            if count >= 2 {
                let source = if first == 0 { cur } else { stages[first - 1].1 };
                return Some(Pipeline { kind: PipelineKind::Array, source, stages: stages.split_off(first) });
            }
        }
        let first = stages.iter().position(|(n, call)| PROMISE_OPS.contains(&n.as_str()) && has_args(*call))?;
        let source = if first == 0 { cur } else { stages[first - 1].1 };
        Some(Pipeline { kind: PipelineKind::Promise, source, stages: stages.split_off(first) })
    }

    fn stage_label(&self, name: &str, call: Node) -> String {
        let args = call.child_by_field_name("arguments").map(|a| self.natural(a)).unwrap_or_else(|| "()".into());
        truncate(&format!("{name}{args}"), 48)
    }

    /// Natural one-line text for an expression; pipelines read as `xs → filter(…) → map(…)`,
    /// and block-bodied functions as `(x) => { … }`.
    pub fn natural(&self, node: Node) -> String {
        let node = unparen(node);
        if let Some(p) = self.pipeline(node) {
            let mut parts = vec![truncate(&self.natural(p.source), 40)];
            parts.extend(p.stages.iter().map(|(n, call)| self.stage_label(n, *call)));
            return parts.join(" → ");
        }
        self.compact(node)
    }

    /// Source text with function bodies longer than a line replaced by `{ … }`.
    fn compact(&self, node: Node) -> String {
        let src = self.src;
        let mut out = String::new();
        let mut pos = node.start_byte();
        let mut stack = vec![node];
        let mut blocks = Vec::new();
        while let Some(n) = stack.pop() {
            if n.kind() == "statement_block" && n.id() != node.id() && n.parent().is_some_and(|p| crate::syntax::is_function(p.kind())) {
                blocks.push(n);
                continue;
            }
            stack.extend(named_children(n));
        }
        blocks.sort_by_key(|b| b.start_byte());
        for b in blocks {
            if b.start_byte() < pos {
                continue;
            }
            out.push_str(&src.text[pos..b.start_byte()]);
            out.push_str(if named_children(b).is_empty() { "{}" } else { "{ … }" });
            pos = b.end_byte();
        }
        out.push_str(&src.text[pos..node.end_byte()]);
        nodex_lang_api::kit::normalize_ws(&out)
    }

    pub fn lower(&mut self, node: Node) -> Expr {
        let src = self.src;
        let node = unparen(node);
        let span = src.span(node);
        let mk = |concept: Concept, label: String, children: Vec<Expr>| Expr {
            concept,
            label,
            span: span.clone(),
            flavors: Vec::new(),
            children,
            symbol: None,
            target: None,
        };
        let text = || truncate(&src.norm(node), 40);
        let with = |mut e: Expr, flavor: &str| {
            e.flavors.push(flavor.to_owned());
            e
        };

        match node.kind() {
            "call_expression" => {
                if let Some(p) = self.pipeline(node) {
                    let mut kids = vec![self.lower(p.source)];
                    for (name, call) in &p.stages {
                        let args = self.arguments(*call);
                        let mut stage = Expr {
                            concept: Concept::Call,
                            label: self.stage_label(name, *call),
                            span: src.span(*call),
                            flavors: vec!["stage".into()],
                            children: args,
                            symbol: None,
                            target: None,
                        };
                        let known = match p.kind {
                            PipelineKind::Array => is_array_stage(name, *call),
                            PipelineKind::Promise => PROMISE_OPS.contains(&name.as_str()),
                        };
                        if !known {
                            stage.flavors.push("custom-stage".into());
                        }
                        kids.push(stage);
                    }
                    let flavor = match p.kind {
                        PipelineKind::Array => "array-pipeline",
                        PipelineKind::Promise => "promise",
                    };
                    return with(mk(Concept::Pipeline, "pipeline".into(), kids), flavor);
                }
                let func = node.child_by_field_name("function");
                let mut kids = Vec::new();
                let mut flavors = Vec::new();
                if children(node).iter().any(|c| c.kind() == "optional_chain") {
                    flavors.push("safe-access".to_owned());
                }
                let label = match func {
                    Some(f) if f.kind() == "member_expression" => {
                        if let Some(obj) = f.child_by_field_name("object") {
                            let mut target = self.lower(obj);
                            target.flavors.push("target".into());
                            kids.push(target);
                        }
                        if children(f).iter().any(|c| c.kind() == "optional_chain") {
                            flavors.push("safe-access".to_owned());
                        }
                        f.child_by_field_name("property").map(|p| src.norm(p)).unwrap_or_default()
                    }
                    Some(f) if f.kind() == "import" => {
                        flavors.extend(["dynamic-import".to_owned(), "async".to_owned()]);
                        "import".to_owned()
                    }
                    Some(f) if f.kind() == "super" => {
                        flavors.push("base-constructor".to_owned());
                        "super".to_owned()
                    }
                    Some(f) => {
                        let callee = src.norm(f);
                        if callee == "require" {
                            flavors.push("commonjs".to_owned());
                        }
                        truncate(&callee, 40)
                    }
                    None => text(),
                };
                if let Some(args) = node.child_by_field_name("arguments") {
                    if args.kind() == "template_string" {
                        flavors.push("tagged-template".to_owned());
                        kids.push(self.lower(args));
                    } else {
                        kids.extend(self.arguments(node));
                    }
                }
                let mut e = mk(Concept::Call, label, kids);
                e.flavors = flavors;
                e
            }
            "new_expression" => {
                let ty = node.child_by_field_name("constructor").map(|c| src.norm(c)).unwrap_or_default();
                self.instantiated.push(bare_type_name(&ty));
                let kids = self.arguments(node);
                mk(Concept::Construct, format!("new {}", truncate(&ty, 32)), kids)
            }
            "member_expression" => {
                let prop = node.child_by_field_name("property").map(|p| src.norm(p)).unwrap_or_default();
                let kids = node.child_by_field_name("object").map(|o| vec![self.lower(o)]).unwrap_or_default();
                if children(node).iter().any(|c| c.kind() == "optional_chain") {
                    mk(Concept::SafeAccess, format!("?.{prop}"), kids)
                } else {
                    mk(Concept::GetMember, format!(".{prop}"), kids)
                }
            }
            "subscript_expression" => {
                let mut kids = Vec::new();
                if let Some(o) = node.child_by_field_name("object") {
                    kids.push(self.lower(o));
                }
                if let Some(i) = node.child_by_field_name("index") {
                    kids.push(self.lower(i));
                }
                let e = mk(Concept::Index, "[ ]".into(), kids);
                if children(node).iter().any(|c| c.kind() == "optional_chain") { with(e, "safe-access") } else { e }
            }
            "binary_expression" => self.binary(node),
            "unary_expression" => {
                let op = node.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_else(|| {
                    children(node).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default()
                });
                let kids = node.child_by_field_name("argument").map(|a| vec![self.lower(a)]).unwrap_or_default();
                let label = match op.as_str() {
                    "!" => "not",
                    "typeof" => "type of",
                    "void" => "discard",
                    "delete" => "delete",
                    other => other,
                };
                let e = mk(Concept::UnaryOp, label.to_owned(), kids);
                if op == "!" { with(e, "truthiness") } else { e }
            }
            "update_expression" => {
                let op = children(node).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
                let kids = node.child_by_field_name("argument").map(|a| vec![self.lower(a)]).unwrap_or_default();
                mk(Concept::UnaryOp, op, kids)
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                let op = node
                    .child_by_field_name("operator")
                    .map(|o| src.text(o).to_owned())
                    .unwrap_or_else(|| "=".into());
                let kids = ["left", "right"].iter().filter_map(|f| node.child_by_field_name(f)).map(|c| self.lower(c)).collect();
                mk(Concept::Assign, op, kids)
            }
            "ternary_expression" => {
                let kids = ["condition", "consequence", "alternative"]
                    .iter()
                    .filter_map(|f| node.child_by_field_name(f))
                    .map(|c| self.lower(c))
                    .collect();
                mk(Concept::Choose, "choose".into(), kids)
            }
            "arrow_function" | "function_expression" | "function" | "generator_function" => self.lambda(node),
            "await_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Await, "await".into(), kids)
            }
            "yield_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let e = mk(Concept::Yield, "yield".into(), kids);
                if children(node).iter().any(|c| src.text(*c) == "*") { with(e, "delegate") } else { e }
            }
            "template_string" => {
                let kids: Vec<Expr> = named_children(node)
                    .into_iter()
                    .filter(|c| c.kind() == "template_substitution")
                    .filter_map(|s| named_children(s).into_iter().next())
                    .map(|c| self.lower(c))
                    .collect();
                if kids.is_empty() {
                    mk(Concept::Literal, text(), Vec::new())
                } else {
                    with(mk(Concept::Compose, "text with values".into(), kids), "interpolation")
                }
            }
            "string" | "number" | "regex" | "true" | "false" | "null" | "undefined" => mk(Concept::Literal, text(), Vec::new()),
            "identifier" | "this" | "super" | "shorthand_property_identifier" | "property_identifier"
            | "private_property_identifier" | "meta_property" | "import" | "statement_identifier" => mk(Concept::Var, text(), Vec::new()),
            "object" => {
                let kids = named_children(node).into_iter().map(|c| self.object_member(c)).collect();
                mk(Concept::Compose, "object".into(), kids)
            }
            "array" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Compose, "list".into(), kids)
            }
            "spread_element" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                with(mk(Concept::Compose, "all items of".into(), kids), "spread")
            }
            "as_expression" | "satisfies_expression" | "type_assertion" => {
                let parts = named_children(node);
                let (value, ty) = if node.kind() == "type_assertion" {
                    (parts.last().copied(), parts.first().copied())
                } else {
                    (parts.first().copied(), parts.last().copied())
                };
                let ty = ty.map(|t| src.norm(t)).unwrap_or_default();
                let ty = ty.trim_start_matches('<').trim_end_matches('>');
                let kids = value.map(|v| vec![self.lower(v)]).unwrap_or_default();
                if node.kind() == "satisfies_expression" {
                    with(mk(Concept::Convert, truncate(&format!("checked as {ty}"), 40), kids), "satisfies")
                } else if ty == "const" {
                    with(mk(Concept::Convert, "as constant".into(), kids), "type-only")
                } else {
                    with(mk(Concept::Convert, truncate(&format!("as {ty}"), 40), kids), "type-only")
                }
            }
            "non_null_expression" => {
                // `x!` changes nothing at run time: show the operand.
                let inner = named_children(node).into_iter().next().unwrap_or(node);
                if inner.id() == node.id() {
                    return mk(Concept::Var, text(), Vec::new());
                }
                with(self.lower(inner), "non-null")
            }
            "instantiation_expression" => {
                let inner = named_children(node).into_iter().next().unwrap_or(node);
                if inner.id() == node.id() {
                    return mk(Concept::Var, text(), Vec::new());
                }
                with(self.lower(inner), "generic")
            }
            "sequence_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Compose, "in order".into(), kids)
            }
            "jsx_element" | "jsx_self_closing_element" => self.jsx(node),
            "jsx_expression" => match named_children(node).into_iter().next() {
                Some(inner) => self.lower(inner),
                None => mk(Concept::Literal, "{}".into(), Vec::new()),
            },
            "jsx_text" => mk(Concept::Literal, text(), Vec::new()),
            "class" => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                with(mk(Concept::Construct, format!("class {name}").trim().to_owned(), Vec::new()), "class-expression")
            }
            "object_pattern" | "array_pattern" => mk(Concept::Destructure, text(), Vec::new()),
            _ => with(mk(Concept::Opaque, text(), Vec::new()), &format!("syntax:{}", node.kind())),
        }
    }

    fn binary(&mut self, node: Node) -> Expr {
        let src = self.src;
        let span = src.span(node);
        let op = node.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_default();
        let left = node.child_by_field_name("left");
        let right = node.child_by_field_name("right");
        let mk = |concept: Concept, label: String, children: Vec<Expr>, flavors: Vec<String>| Expr {
            concept,
            label,
            span: span.clone(),
            flavors,
            children,
            symbol: None,
            target: None,
        };
        // `typeof x === "string"`: a type check that narrows `x`.
        if matches!(op.as_str(), "===" | "==" | "!==" | "!=")
            && let (Some(l), Some(r)) = (left.map(unparen), right.map(unparen))
        {
            let (probe, lit) = if l.kind() == "unary_expression" && r.kind() == "string" { (l, r) } else { (r, l) };
            if probe.kind() == "unary_expression"
                && lit.kind() == "string"
                && probe.child_by_field_name("operator").is_some_and(|o| src.text(o) == "typeof")
                && let Some(arg) = probe.child_by_field_name("argument")
            {
                let negated = op.starts_with('!');
                let label = format!("{}is {}", if negated { "not " } else { "" }, string_value(src, lit));
                return mk(Concept::TypeCheck, label, vec![self.lower(arg)], vec!["typeof".into()]);
            }
        }
        let kids: Vec<Expr> = [left, right].into_iter().flatten().map(|c| self.lower(c)).collect();
        match op.as_str() {
            "??" => mk(Concept::Fallback, "fallback".into(), kids, Vec::new()),
            "instanceof" => {
                let ty = right.map(|r| src.norm(r)).unwrap_or_default();
                let kids = left.map(|l| vec![self.lower(l)]).unwrap_or_default();
                mk(Concept::TypeCheck, truncate(&format!("is {ty}"), 40), kids, vec!["instanceof".into()])
            }
            "in" => mk(Concept::TypeCheck, "has key".into(), kids, vec!["in".into()]),
            "==" | "!=" => mk(
                Concept::BinaryOp,
                if op == "==" { "==".into() } else { "≠".into() },
                kids,
                vec!["loose-equality".into()],
            ),
            "&&" | "||" => mk(Concept::BinaryOp, if op == "&&" { "and".into() } else { "or".into() }, kids, Vec::new()),
            _ => mk(Concept::BinaryOp, op_label(&op).to_owned(), kids, Vec::new()),
        }
    }

    fn lambda(&mut self, node: Node) -> Expr {
        let src = self.src;
        let params = match node.child_by_field_name("parameter") {
            Some(p) => src.norm(p),
            None => node.child_by_field_name("parameters").map(|p| src.norm(p)).unwrap_or_default(),
        };
        let body = node.child_by_field_name("body");
        let kids = match body {
            Some(b) if b.kind() != "statement_block" => vec![self.lower(b)],
            _ => Vec::new(),
        };
        let mut e = Expr {
            concept: Concept::Lambda,
            label: truncate(&format!("({}) => …", params.trim_matches(['(', ')'])), 40),
            span: src.span(node),
            flavors: Vec::new(),
            children: kids,
            symbol: None,
            target: None,
        };
        if children(node).iter().any(|c| !c.is_named() && src.text(*c) == "async") {
            e.flavors.push("async".into());
        }
        if node.kind() != "arrow_function" {
            if node.kind() == "generator_function" {
                e.flavors.push("generator".into());
            }
            if body.is_some_and(uses_this) {
                e.flavors.push("this-binding".into());
            }
        }
        e
    }

    /// A member of an object literal: `key: value` keeps its key in the label.
    fn object_member(&mut self, node: Node) -> Expr {
        let src = self.src;
        match node.kind() {
            "pair" => {
                let key = node.child_by_field_name("key").map(|k| src.norm(k)).unwrap_or_default();
                match node.child_by_field_name("value") {
                    Some(v) => {
                        let mut e = self.lower(v);
                        e.label = truncate(&format!("{key}: {}", e.label), 48);
                        e
                    }
                    None => self.lower(node),
                }
            }
            "method_definition" => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let mut e = self.lambda(node);
                e.label = truncate(&format!("{name}: {}", e.label), 48);
                e
            }
            _ => self.lower(node),
        }
    }

    fn jsx(&mut self, node: Node) -> Expr {
        let src = self.src;
        let open = if node.kind() == "jsx_element" { node.child_by_field_name("open_tag") } else { Some(node) };
        let name = open.and_then(|o| o.child_by_field_name("name")).map(|n| src.norm(n));
        let mut kids = Vec::new();
        if let Some(o) = open {
            for attr in named_children(o) {
                match attr.kind() {
                    "jsx_attribute" => {
                        let parts = named_children(attr);
                        let key = parts.first().map(|k| src.norm(*k)).unwrap_or_default();
                        match parts.get(1) {
                            Some(v) => {
                                let mut e = self.lower(*v);
                                e.label = truncate(&format!("{key}: {}", e.label), 48);
                                kids.push(e);
                            }
                            None => kids.push(Expr {
                                concept: Concept::Literal,
                                label: format!("{key}: true"),
                                span: src.span(attr),
                                flavors: Vec::new(),
                                children: Vec::new(),
                                symbol: None,
                                target: None,
                            }),
                        }
                    }
                    "jsx_expression" => kids.push(self.lower(attr)),
                    _ => {}
                }
            }
        }
        if node.kind() == "jsx_element" {
            for child in named_children(node) {
                match child.kind() {
                    "jsx_opening_element" | "jsx_closing_element" => {}
                    "jsx_text" if src.text(child).trim().is_empty() => {}
                    "jsx_expression" if named_children(child).is_empty() => {}
                    _ => kids.push(self.lower(child)),
                }
            }
        }
        let label = match &name {
            Some(n) => format!("<{n}>"),
            None => "fragment".to_owned(),
        };
        if let Some(n) = &name
            && n.starts_with(char::is_uppercase)
        {
            self.instantiated.push(n.clone());
        }
        Expr {
            concept: Concept::Construct,
            label: truncate(&label, 40),
            span: src.span(node),
            flavors: vec!["jsx".into()],
            children: kids,
            symbol: None,
            target: None,
        }
    }

    fn arguments(&mut self, call: Node) -> Vec<Expr> {
        call.child_by_field_name("arguments")
            .filter(|a| a.kind() == "arguments")
            .map(|a| named_children(a).into_iter().map(|c| self.lower(c)).collect())
            .unwrap_or_default()
    }
}

fn op_label(op: &str) -> &str {
    match op {
        "===" => "==",
        "!==" => "≠",
        "<=" => "≤",
        ">=" => "≥",
        other => other,
    }
}

/// Names of identifiers read by `node` that may refer to locals or parameters. Property names,
/// type names and nested function parameters are excluded.
pub fn reads(src: Src, node: Node) -> Vec<String> {
    let mut out = Vec::new();
    collect_reads(src, node, &mut Vec::new(), &mut out);
    out
}

fn collect_reads(src: Src, node: Node, shadowed: &mut Vec<String>, out: &mut Vec<String>) {
    match node.kind() {
        "identifier" | "shorthand_property_identifier" => {
            let name = src.text(node).to_owned();
            if !shadowed.contains(&name) && !out.contains(&name) && !is_binding(node) {
                out.push(name);
            }
            return;
        }
        k if is_function_value(k) || k == "method_definition" => {
            let before = shadowed.len();
            if let Some(p) = node.child_by_field_name("parameter") {
                shadowed.push(src.text(p).to_owned());
            }
            if let Some(params) = node.child_by_field_name("parameters") {
                for p in named_children(params) {
                    let pattern = p.child_by_field_name("pattern").unwrap_or(p);
                    pattern_names(src, pattern, shadowed);
                }
            }
            if let Some(body) = node.child_by_field_name("body") {
                collect_reads(src, body, shadowed, out);
            }
            shadowed.truncate(before);
            return;
        }
        "type_annotation" | "type_arguments" | "type_parameters" | "jsx_closing_element" => return,
        _ => {}
    }
    for child in named_children(node) {
        collect_reads(src, child, shadowed, out);
    }
}

/// True for identifiers that declare a name rather than read one.
fn is_binding(node: Node) -> bool {
    let Some(parent) = node.parent() else { return false };
    let field_is = |field: &str| parent.child_by_field_name(field).is_some_and(|f| f.id() == node.id());
    match parent.kind() {
        "variable_declarator" => field_is("name"),
        "required_parameter" | "optional_parameter" => field_is("pattern"),
        "function_declaration" | "generator_function_declaration" | "class_declaration" | "function_expression"
        | "class" => field_is("name"),
        "catch_clause" => field_is("parameter"),
        "for_in_statement" => field_is("left"),
        // Tag names (`<Button>`, `<div>`) name components and elements, not data.
        "jsx_opening_element" | "jsx_self_closing_element" => field_is("name"),
        "array_pattern" | "object_pattern" | "rest_pattern" | "pair_pattern" | "assignment_pattern" => true,
        "labeled_statement" | "break_statement" | "continue_statement" => true,
        _ => false,
    }
}
