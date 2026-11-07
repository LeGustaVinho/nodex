//! Expressions → [`Expr`] trees, natural-language text and local-variable reads.

use nodex_ir::{Concept, Expr};
use nodex_lang_api::kit::truncate;
use nodex_lang_api::ops::{self, Equality};
use tree_sitter::Node;

use crate::syntax::{Src, bare_type_name, children, first_child_of_kind, named_children};

/// LINQ operators. A method chain with two or more of these is shown as a `Pipeline`.
const LINQ: &[&str] = &[
    "Where", "Select", "SelectMany", "OrderBy", "OrderByDescending", "ThenBy", "ThenByDescending",
    "GroupBy", "Join", "GroupJoin", "Zip", "Distinct", "DistinctBy", "Union", "Intersect", "Except",
    "Take", "TakeWhile", "Skip", "SkipWhile", "Reverse", "Concat", "Append", "Prepend", "Chunk",
    "Sum", "Count", "LongCount", "Min", "MinBy", "Max", "MaxBy", "Average", "Aggregate", "Any", "All",
    "Contains", "First", "FirstOrDefault", "Last", "LastOrDefault", "Single", "SingleOrDefault",
    "ElementAt", "ToList", "ToArray", "ToDictionary", "ToHashSet", "ToLookup", "Cast", "OfType",
    "AsEnumerable", "AsQueryable", "DefaultIfEmpty",
];

/// LINQ operators that always take an argument. Without one (`repo.All()`), the name is more
/// likely a project method than a LINQ stage.
const LINQ_NEEDS_ARGS: &[&str] = &[
    "Where", "Select", "SelectMany", "OrderBy", "OrderByDescending", "ThenBy", "ThenByDescending", "GroupBy",
    "Join", "GroupJoin", "Zip", "All", "Aggregate", "Contains", "TakeWhile", "SkipWhile", "Take", "Skip",
    "MinBy", "MaxBy", "DistinctBy", "Chunk", "ElementAt", "Append", "Prepend", "Concat", "Union", "Intersect", "Except",
];

fn is_linq_stage(_src: Src, name: &str, invocation: Node) -> bool {
    if !LINQ.contains(&name) {
        return false;
    }
    if LINQ_NEEDS_ARGS.contains(&name) {
        let has_args = invocation
            .child_by_field_name("arguments")
            .is_some_and(|a| named_children(a).iter().any(|c| c.kind() == "argument"));
        return has_args;
    }
    true
}

/// Lowers expressions and remembers which types were instantiated.
pub struct ExprLowering<'a> {
    pub src: Src<'a>,
    pub instantiated: Vec<String>,
    /// Lambdas with a Blueprint of their own, by start byte (`Expr::symbol` of the `Lambda`).
    pub functions: std::collections::HashMap<usize, nodex_ir::SymbolId>,
}

/// A method-chain pipeline: `source.Where(..).Select(..).Sum()`.
pub struct Pipeline<'t> {
    pub source: Node<'t>,
    /// `(operator name, invocation node)` from first to last.
    pub stages: Vec<(String, Node<'t>)>,
}

impl<'a> ExprLowering<'a> {
    pub fn new(src: Src<'a>) -> Self {
        Self { src, instantiated: Vec::new(), functions: Default::default() }
    }

    /// Detects a LINQ-style pipeline rooted at `node`.
    pub fn pipeline<'t>(&self, node: Node<'t>) -> Option<Pipeline<'t>> {
        let mut stages = Vec::new();
        let mut cur = node;
        while cur.kind() == "invocation_expression" {
            let Some(func) = cur.child_by_field_name("function") else { break };
            if func.kind() != "member_access_expression" {
                break;
            }
            let Some(name) = func.child_by_field_name("name") else { break };
            let name = bare_type_name(self.src.text(name));
            stages.push((name, cur));
            let Some(receiver) = func.child_by_field_name("expression") else { break };
            cur = receiver;
        }
        stages.reverse();
        let src = self.src;
        let first_linq = stages.iter().position(|(n, inv)| is_linq_stage(src, n, *inv))?;
        let linq_count = stages[first_linq..].iter().filter(|(n, inv)| is_linq_stage(src, n, *inv)).count();
        if linq_count < 2 {
            return None;
        }
        let source = if first_linq == 0 {
            cur
        } else {
            stages[first_linq - 1].1
        };
        Some(Pipeline { source, stages: stages.split_off(first_linq) })
    }

    /// Short label of one pipeline stage: `Where(o => o.Customer == customer)`.
    fn stage_label(&self, name: &str, invocation: Node) -> String {
        let args = invocation
            .child_by_field_name("arguments")
            .map(|a| self.src.norm(a))
            .unwrap_or_else(|| "()".into());
        truncate(&format!("{name}{args}"), 48)
    }

    /// Natural one-line text for an expression; pipelines read as `source → Where(…) → Sum()`.
    pub fn natural(&self, node: Node) -> String {
        let node = unparen(node);
        if let Some(p) = self.pipeline(node) {
            let mut parts = vec![truncate(&self.src.norm(p.source), 40)];
            parts.extend(p.stages.iter().map(|(n, inv)| self.stage_label(n, *inv)));
            return parts.join(" → ");
        }
        if node.kind() == "switch_expression" {
            let value = named_children(node).into_iter().find(|c| c.kind() != "switch_expression_arm");
            let arms = named_children(node).into_iter().filter(|c| c.kind() == "switch_expression_arm").count();
            return format!(
                "match {} ({arms} cases)",
                value.map(|v| self.natural(v)).unwrap_or_default()
            );
        }
        self.src.norm(node)
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
            op: None,
            ty: None,
            params: Vec::new(),
        };
        let text = || truncate(&src.norm(node), 40);

        match node.kind() {
            "invocation_expression" => {
                if let Some(p) = self.pipeline(node) {
                    let mut kids = vec![self.lower(p.source)];
                    for (name, inv) in &p.stages {
                        let args = self.arguments(*inv);
                        let mut stage = Expr {
                            concept: Concept::Call,
                            label: self.stage_label(name, *inv),
                            span: src.span(*inv),
                            flavors: vec!["stage".into()],
                            children: args,
                            symbol: None,
                            target: None,
                            op: None,
                            ty: None,
                            params: Vec::new(),
                        };
                        if !is_linq_stage(src, name, *inv) {
                            stage.flavors.push("custom-stage".into());
                        }
                        kids.push(stage);
                    }
                    let mut e = mk(Concept::Pipeline, "pipeline".into(), kids);
                    e.flavors.push("linq".into());
                    return e;
                }
                let func = node.child_by_field_name("function");
                let callee = func.map(|f| src.norm(f)).unwrap_or_default();
                if callee == "nameof" {
                    return mk(Concept::Literal, text(), Vec::new());
                }
                let mut kids = Vec::new();
                let mut flavors = Vec::new();
                if let Some(f) = func {
                    match f.kind() {
                        "member_access_expression" => {
                            if let Some(recv) = f.child_by_field_name("expression") {
                                let mut target = self.lower(recv);
                                target.flavors.push("target".into());
                                kids.push(target);
                            }
                        }
                        "conditional_access_expression" => flavors.push("safe-access".into()),
                        "generic_name" => flavors.push("generic".into()),
                        _ => {}
                    }
                }
                kids.extend(self.arguments(node));
                let label = match func {
                    Some(f) if f.kind() == "member_access_expression" => f
                        .child_by_field_name("name")
                        .map(|n| src.norm(n))
                        .unwrap_or(callee),
                    _ => truncate(&callee, 40),
                };
                let mut e = mk(Concept::Call, label, kids);
                e.flavors = flavors;
                e
            }
            "member_access_expression" => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let kids = node.child_by_field_name("expression").map(|e| vec![self.lower(e)]).unwrap_or_default();
                mk(Concept::GetMember, format!(".{name}"), kids)
            }
            "member_binding_expression" => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                mk(Concept::GetMember, format!(".{name}"), Vec::new())
            }
            "conditional_access_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::SafeAccess, "?.".into(), kids)
            }
            "element_access_expression" | "element_binding_expression" => {
                let mut kids = Vec::new();
                if let Some(e) = node.child_by_field_name("expression") {
                    kids.push(self.lower(e));
                }
                if let Some(sub) = node.child_by_field_name("subscript").or_else(|| first_child_of_kind(node, "bracketed_argument_list")) {
                    kids.extend(self.argument_list(sub));
                }
                mk(Concept::Index, "[ ]".into(), kids)
            }
            "binary_expression" => {
                let op = node.child_by_field_name("operator").map(|o| src.norm(o)).unwrap_or_default();
                let kids: Vec<Expr> = ["left", "right"]
                    .iter()
                    .filter_map(|f| node.child_by_field_name(f))
                    .map(|c| self.lower(c))
                    .collect();
                let concept = match op.as_str() {
                    "??" => Concept::Fallback,
                    "as" => Concept::Convert,
                    "is" => Concept::TypeCheck,
                    _ => Concept::BinaryOp,
                };
                let e = mk(concept, op_label(&op).to_owned(), kids);
                if concept == Concept::BinaryOp { ops::operator(e, &op, Equality::Strict) } else { e }
            }
            "is_pattern_expression" => {
                let pattern = node.child_by_field_name("pattern").map(|p| src.norm(p)).unwrap_or_default();
                let kids = node.child_by_field_name("expression").map(|e| vec![self.lower(e)]).unwrap_or_default();
                mk(Concept::TypeCheck, truncate(&format!("is {pattern}"), 40), kids)
            }
            "postfix_unary_expression" if children(node).iter().any(|c| !c.is_named() && src.text(*c) == "!") => {
                // `x!` (null-forgiving) changes nothing at run time: show the operand.
                let inner = named_children(node).into_iter().next().unwrap_or(node);
                let mut e = self.lower(inner);
                e.flavors.push("null-forgiving".into());
                e
            }
            "ref_expression" | "checked_expression" => {
                let inner = named_children(node).into_iter().next();
                let flavor = match node.kind() {
                    "ref_expression" => "ref",
                    _ if children(node).iter().any(|c| src.text(*c) == "unchecked") => "unchecked",
                    _ => "checked",
                };
                match inner {
                    Some(i) => {
                        let mut e = self.lower(i);
                        e.flavors.push(flavor.into());
                        e
                    }
                    None => mk(Concept::Opaque, text(), Vec::new()),
                }
            }
            "range_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = mk(Concept::Compose, format!("range {}", text()), kids);
                e.flavors.push("range".into());
                e
            }
            "as_expression" => {
                let ty = node.child_by_field_name("right").or_else(|| named_children(node).into_iter().last()).map(|t| src.norm(t)).unwrap_or_default();
                let kids = node.child_by_field_name("left").or_else(|| named_children(node).into_iter().next()).map(|v| vec![self.lower(v)]).unwrap_or_default();
                let mut e = mk(Concept::Convert, format!("as {ty}"), kids);
                e.flavors.push("safe-cast".into());
                e
            }
            "is_expression" => {
                let ty = node.child_by_field_name("right").map(|t| src.norm(t)).unwrap_or_default();
                let kids = node.child_by_field_name("left").map(|v| vec![self.lower(v)]).unwrap_or_default();
                mk(Concept::TypeCheck, format!("is {ty}"), kids)
            }
            "spread_element" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = mk(Concept::Compose, "all items of".into(), kids);
                e.flavors.push("spread".into());
                e
            }
            "prefix_unary_expression" | "postfix_unary_expression" => {
                let op = children(node).into_iter().find(|c| !c.is_named()).map(|c| src.norm(c)).unwrap_or_default();
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = ops::operator(mk(Concept::UnaryOp, op_label(&op).to_owned(), kids), &op, Equality::Strict);
                if matches!(op.as_str(), "++" | "--") {
                    e.flavors.push(if node.kind() == "prefix_unary_expression" { "prefix" } else { "postfix" }.into());
                }
                e
            }
            "assignment_expression" => {
                let op = node.child_by_field_name("operator").map(|o| src.norm(o)).unwrap_or_else(|| "=".into());
                let kids = ["left", "right"]
                    .iter()
                    .filter_map(|f| node.child_by_field_name(f))
                    .map(|c| self.lower(c))
                    .collect();
                let e = mk(Concept::Assign, op.clone(), kids);
                ops::compound_assign(e, &op, Equality::Strict)
            }
            "conditional_expression" => {
                let kids = ["condition", "consequence", "alternative"]
                    .iter()
                    .filter_map(|f| node.child_by_field_name(f))
                    .map(|c| self.lower(c))
                    .collect();
                mk(Concept::Choose, "choose".into(), kids)
            }
            "object_creation_expression" => {
                let ty = node.child_by_field_name("type").map(|t| src.norm(t)).unwrap_or_default();
                self.instantiated.push(bare_type_name(&ty));
                let mut kids = self.arguments(node);
                if let Some(init) = node.child_by_field_name("initializer") {
                    kids.push(self.lower(init));
                }
                mk(Concept::Construct, format!("new {}", truncate(&ty, 32)), kids)
            }
            "implicit_object_creation_expression" => {
                let kids = self.arguments(node);
                mk(Concept::Construct, "new()".into(), kids)
            }
            "with_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = mk(Concept::Construct, "copy with changes".into(), kids);
                e.flavors.push("copy-with".into());
                e
            }
            "with_initializer" | "initializer_expression" | "array_creation_expression"
            | "implicit_array_creation_expression" | "collection_expression" | "tuple_expression"
            | "anonymous_object_creation_expression" | "stackalloc_expression" => {
                let kids = named_children(node)
                    .into_iter()
                    .filter(|c| !c.kind().ends_with("type") && c.kind() != "array_rank_specifier")
                    .map(|c| self.lower_element(c))
                    .collect();
                mk(Concept::Compose, compose_label(node.kind()).into(), kids)
            }
            "interpolated_string_expression" => {
                let kids = named_children(node)
                    .into_iter()
                    .filter(|c| c.kind() == "interpolation")
                    .filter_map(|i| named_children(i).into_iter().find(|c| !c.kind().starts_with("interpolation")))
                    .map(|c| self.lower(c))
                    .collect();
                let mut e = mk(Concept::Compose, "text with values".into(), kids);
                e.flavors.push("interpolation".into());
                e.ty = Some("text".into());
                e
            }
            "string_literal" | "verbatim_string_literal" | "raw_string_literal" | "integer_literal"
            | "real_literal" | "boolean_literal" | "null_literal" | "character_literal"
            | "typeof_expression" | "default_expression" | "sizeof_expression" => {
                let mut e = mk(Concept::Literal, text(), Vec::new());
                e.ty = ops::literal_type(&e.label).map(str::to_owned);
                e
            }
            "identifier" | "this" | "base" | "this_expression" | "base_expression" | "generic_name"
            | "qualified_name" | "predefined_type" | "alias_qualified_name" | "member_binding_expression_name" => {
                mk(Concept::Var, text(), Vec::new())
            }
            "implicit_stackalloc_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower_element(c)).collect();
                mk(Concept::Compose, "stack array".into(), kids)
            }
            "makeref_expression" | "reftype_expression" | "refvalue_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Call, text(), kids)
            }
            // `#if` inside an initializer or argument list: all variants, tagged compile-time.
            "preproc_if" | "preproc_elif" | "preproc_else" => {
                let mut kids: Vec<Expr> = crate::syntax::preproc_body(node).into_iter().map(|c| self.lower_element(c)).collect();
                if let Some(alt) = node.child_by_field_name("alternative") {
                    kids.push(self.lower(alt));
                }
                let mut e = mk(Concept::Compose, crate::syntax::preproc_label(src, node), kids);
                e.flavors.push("compile-time".into());
                e
            }
            "declaration_expression" => {
                let mut e = mk(Concept::Var, text(), Vec::new());
                e.flavors.push("out-declaration".into());
                e
            }
            "lambda_expression" | "anonymous_method_expression" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| src.norm(p))
                    .unwrap_or_default();
                let body = node.child_by_field_name("body");
                let kids = match body {
                    Some(b) if b.kind() != "block" => vec![self.lower(b)],
                    _ => Vec::new(),
                };
                let mut e = mk(Concept::Lambda, truncate(&format!("({}) => …", params.trim_matches(['(', ')'])), 40), kids);
                e.params = ops::param_names(&params);
                e.symbol = self.functions.get(&node.start_byte()).cloned();
                if children(node).iter().any(|c| src.text(*c) == "async") {
                    e.flavors.push("async".into());
                }
                e
            }
            "await_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Await, "await".into(), kids)
            }
            "throw_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                mk(Concept::Throw, "throw".into(), kids)
            }
            "cast_expression" => {
                let ty = node.child_by_field_name("type").map(|t| src.norm(t)).unwrap_or_default();
                let kids = node.child_by_field_name("value").map(|v| vec![self.lower(v)]).unwrap_or_default();
                mk(Concept::Convert, format!("as {ty}"), kids)
            }
            "switch_expression" => {
                let mut kids = Vec::new();
                for c in named_children(node) {
                    if c.kind() == "switch_expression_arm" {
                        let parts = named_children(c);
                        let (result, patterns) = parts.split_last().map(|(r, p)| (Some(*r), p.to_vec())).unwrap_or((None, Vec::new()));
                        let pattern = patterns.iter().map(|p| src.norm(*p)).collect::<Vec<_>>().join(" when ");
                        let arm_kids = result.map(|r| vec![self.lower(r)]).unwrap_or_default();
                        kids.push(Expr {
                            concept: Concept::Match,
                            label: truncate(&format!("{pattern} ⇒"), 40),
                            span: src.span(c),
                            flavors: vec!["arm".into()],
                            children: arm_kids,
                            symbol: None,
                            target: None,
                            op: None,
                            ty: None,
                            params: Vec::new(),
                        });
                    } else {
                        kids.push(self.lower(c));
                    }
                }
                mk(Concept::Match, "match".into(), kids)
            }
            "query_expression" => {
                let kids = named_children(node)
                    .into_iter()
                    .map(|clause| Expr {
                        concept: Concept::Call,
                        label: truncate(&src.norm(clause), 48),
                        span: src.span(clause),
                        flavors: vec!["stage".into()],
                        children: Vec::new(),
                        symbol: None,
                        target: None,
                        op: None,
                        ty: None,
                        params: Vec::new(),
                    })
                    .collect();
                let mut e = mk(Concept::Pipeline, "pipeline".into(), kids);
                e.flavors.extend(["linq".to_owned(), "query-syntax".to_owned()]);
                e
            }
            "variable_declaration" => {
                // `using (var x = new R())`: show the initial values.
                let values: Vec<Expr> = named_children(node)
                    .into_iter()
                    .filter(|c| c.kind() == "variable_declarator")
                    .filter_map(|d| named_children(d).into_iter().nth(1))
                    .map(|v| self.lower(v))
                    .collect();
                if values.len() == 1 {
                    values.into_iter().next().unwrap_or_else(|| mk(Concept::Opaque, text(), Vec::new()))
                } else {
                    mk(Concept::Compose, "declarations".into(), values)
                }
            }
            _ => {
                let mut e = mk(Concept::Opaque, text(), Vec::new());
                e.flavors.push(format!("syntax:{}", node.kind()));
                e
            }
        }
    }

    /// An element of a tuple, collection or initializer; `argument` wrappers (`Name: value`) are
    /// unwrapped and their name kept in the label.
    fn lower_element(&mut self, node: Node) -> Expr {
        // `[.. xs, 4]`: collection_element → spread_element / expression_element → value.
        if matches!(node.kind(), "collection_element" | "expression_element") {
            return match named_children(node).into_iter().next() {
                Some(inner) => self.lower_element(inner),
                None => self.lower(node),
            };
        }
        if node.kind() != "argument" {
            return self.lower(node);
        }
        let name = node
            .child_by_field_name("name")
            .or_else(|| first_child_of_kind(node, "name_colon"))
            .map(|n| self.src.norm(n).trim_end_matches(':').trim().to_owned());
        match named_children(node).into_iter().rfind(|c| c.kind() != "name_colon") {
            Some(value) => {
                let mut e = self.lower(value);
                if let Some(n) = name {
                    e.label = format!("{n}: {}", e.label);
                }
                e
            }
            None => self.lower(node),
        }
    }

    fn arguments(&mut self, invocation: Node) -> Vec<Expr> {
        invocation
            .child_by_field_name("arguments")
            .or_else(|| first_child_of_kind(invocation, "argument_list"))
            .map(|a| self.argument_list(a))
            .unwrap_or_default()
    }

    fn argument_list(&mut self, list: Node) -> Vec<Expr> {
        named_children(list)
            .into_iter()
            .filter(|a| a.kind() == "argument")
            .filter_map(|a| named_children(a).into_iter().last())
            .map(|e| self.lower(e))
            .collect()
    }
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

fn op_label(op: &str) -> &str {
    match op {
        "&&" => "and",
        "||" => "or",
        "!" => "not",
        "==" => "==",
        "!=" => "≠",
        "<=" => "≤",
        ">=" => "≥",
        "??" => "fallback",
        other => other,
    }
}

fn compose_label(kind: &str) -> &'static str {
    match kind {
        "tuple_expression" => "tuple",
        "anonymous_object_creation_expression" => "anonymous object",
        "with_initializer" | "initializer_expression" => "initial values",
        _ => "collection",
    }
}

/// Names of identifiers read by `node` that may refer to locals or parameters. Member names,
/// type names and lambda parameters are excluded.
pub fn reads(src: Src, node: Node) -> Vec<String> {
    let mut out = Vec::new();
    collect_reads(src, node, &mut Vec::new(), &mut out);
    out
}

fn collect_reads(src: Src, node: Node, shadowed: &mut Vec<String>, out: &mut Vec<String>) {
    match node.kind() {
        "identifier" => {
            let name = src.text(node).to_owned();
            if !shadowed.contains(&name) && !out.contains(&name) && !is_non_value_position(node) {
                out.push(name);
            }
            return;
        }
        "lambda_expression" | "anonymous_method_expression" => {
            let before = shadowed.len();
            if let Some(params) = node.child_by_field_name("parameters") {
                collect_param_names(src, params, shadowed);
            }
            if let Some(body) = node.child_by_field_name("body") {
                collect_reads(src, body, shadowed, out);
            }
            shadowed.truncate(before);
            return;
        }
        "generic_name" | "qualified_name" | "predefined_type" | "type_argument_list" | "attribute_list" => return,
        _ => {}
    }
    for child in named_children(node) {
        collect_reads(src, child, shadowed, out);
    }
}

fn collect_param_names(src: Src, params: Node, out: &mut Vec<String>) {
    match params.kind() {
        "implicit_parameter" | "identifier" => out.push(src.text(params).to_owned()),
        _ => {
            for p in named_children(params) {
                if let Some(name) = p.child_by_field_name("name") {
                    out.push(src.text(name).to_owned());
                } else if matches!(p.kind(), "identifier" | "implicit_parameter") {
                    out.push(src.text(p).to_owned());
                }
            }
        }
    }
}

/// True for identifiers that name members or types rather than values.
fn is_non_value_position(node: Node) -> bool {
    let Some(parent) = node.parent() else { return false };
    let field_is = |field: &str| parent.child_by_field_name(field).is_some_and(|f| f.id() == node.id());
    match parent.kind() {
        "member_access_expression" | "member_binding_expression" => field_is("name"),
        "object_creation_expression" | "cast_expression" | "variable_declaration" | "declaration_expression"
        | "typeof_expression" | "default_expression" | "is_expression" | "local_function_statement" => field_is("type") || field_is("name"),
        "argument" => field_is("name"),
        "variable_declarator" | "parameter" | "catch_declaration" | "type_parameter" => field_is("name") || field_is("type"),
        "foreach_statement" => field_is("left") || field_is("type"),
        "name_colon" | "name_equals" | "attribute" | "goto_statement" | "labeled_statement" | "nullable_type"
        | "array_type" | "pointer_type" | "ref_type" | "type_pattern" | "declaration_pattern" | "recursive_pattern" => true,
        "with_initializer" => parent.named_child(0).is_some_and(|f| f.id() == node.id()),
        _ => false,
    }
}
