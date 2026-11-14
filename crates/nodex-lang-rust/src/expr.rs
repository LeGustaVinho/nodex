//! Expressions → [`Expr`] trees, natural-language text and local-variable reads (with how each
//! read hands the value over: move, copy, borrow).

use std::collections::HashMap;

use nodex_ir::{Access, Concept, Expr, Focus, SymbolId};
use nodex_lang_api::kit::truncate;
use nodex_lang_api::ops::{self, Equality};
use tree_sitter::Node;

use crate::syntax::{Src, block_statements, children, first_child_of_kind, inclusive_range_to, is_expression, last_segment, named_children, parameters, pattern_bindings, statement_tail, unparen};

pub(crate) fn reference_focus(src: Src, node: Node) -> Option<Focus> {
    let mut node = unparen(node);
    if node.kind() == "generic_function" {
        node = node.child_by_field_name("function")?;
    }
    let member = match node.kind() {
        "field_expression" => node.child_by_field_name("field"),
        "scoped_identifier" | "scoped_type_identifier" => node.child_by_field_name("name"),
        _ => None,
    };
    member.map_or_else(|| src.focus(node), |member| src.member_focus(node, member))
}

fn type_focus(src: Src, operand: Node) -> Option<Focus> {
    let mut ty = operand;
    while matches!(ty.kind(), "generic_type" | "generic_type_with_turbofish" | "struct_pattern" | "tuple_struct_pattern") {
        let Some(inner) = ty.child_by_field_name("type") else { break };
        ty = inner;
    }
    if matches!(ty.kind(), "scoped_identifier" | "scoped_type_identifier")
        && let Some(name) = ty.child_by_field_name("name")
    {
        return src.split_focus(operand.start_byte(), name.start_byte(), operand.end_byte());
    }
    src.focus(operand)
}

fn expression_focus(src: Src, node: Node, concept: Concept) -> Option<Focus> {
    let operand = match concept {
        Concept::Var | Concept::GetMember | Concept::Index => return reference_focus(src, node),
        Concept::Call if node.kind() == "macro_invocation" => {
            let mut focus = reference_focus(src, node.child_by_field_name("macro")?)?;
            focus.primary.push('!');
            focus.primary_bytes = focus.primary_bytes.and_then(|[start, end]| {
                (src.text.as_bytes().get(end as usize) == Some(&b'!')).then_some([start, end + 1])
            });
            return Some(focus);
        }
        Concept::Call => {
            let callee = unparen(node.child_by_field_name("function")?);
            if callee.kind() == "closure_expression" {
                return None;
            }
            return reference_focus(src, callee);
        }
        Concept::Construct if node.kind() == "call_expression" => {
            let mut callee = node.child_by_field_name("function")?;
            if callee.kind() == "generic_function" {
                callee = callee.child_by_field_name("function")?;
            }
            return type_focus(src, callee.child_by_field_name("path")?);
        }
        Concept::Construct => return type_focus(src, node.child_by_field_name("name")?),
        Concept::Assign => return reference_focus(src, node.child_by_field_name("left")?),
        Concept::Choose => node.child_by_field_name("condition"),
        Concept::Convert => return type_focus(src, node.child_by_field_name("type")?),
        Concept::TypeCheck => return type_focus(src, node.child_by_field_name("pattern")?),
        Concept::Match if node.kind() == "match_expression" => node.child_by_field_name("value"),
        Concept::Await | Concept::Return | Concept::Yield => named_children(node).into_iter().next(),
        _ => None,
    };
    src.focus(unparen(operand?))
}

/// Iterator adapters and consumers. A method chain with two or more of these is a `Pipeline`.
const ITER_OPS: &[&str] = &[
    "iter", "into_iter", "iter_mut", "drain", "chars", "bytes", "lines", "split_whitespace", "keys", "values", "values_mut",
    "filter", "map", "filter_map", "flat_map", "flatten", "fold", "try_fold", "reduce", "sum", "product", "count",
    "collect", "take", "skip", "take_while", "skip_while", "map_while", "step_by", "enumerate", "zip", "unzip", "chain",
    "rev", "any", "all", "find", "find_map", "position", "rposition", "min", "max", "min_by", "max_by", "min_by_key",
    "max_by_key", "for_each", "try_for_each", "inspect", "peekable", "cloned", "copied", "last", "nth", "partition",
    "scan", "windows", "chunks", "cycle", "fuse", "by_ref", "sorted", "dedup", "group_by", "join",
];

/// Iterator stages that are only meaningful after another stage (a lone `.map()` on an
/// `Option` is not a pipeline).
const SOURCES: &[&str] = &["iter", "into_iter", "iter_mut", "drain", "chars", "bytes", "lines", "split_whitespace", "keys", "values", "values_mut"];

/// Macros that only format their arguments (they borrow them).
const FORMAT_MACROS: &[&str] = &[
    "println", "print", "eprintln", "eprint", "format", "write", "writeln", "panic", "assert", "assert_eq", "assert_ne",
    "debug_assert", "debug_assert_eq", "debug_assert_ne", "unreachable", "todo", "unimplemented", "format_args", "dbg",
    "info", "warn", "error", "debug", "trace", "log",
];

/// A method-chain pipeline: `orders.iter().filter(..).map(..).sum()`.
pub struct Pipeline<'t> {
    pub source: Node<'t>,
    /// `(stage name, call node)` from first to last.
    pub stages: Vec<(String, Node<'t>)>,
}

pub struct ExprLowering<'a> {
    pub src: Src<'a>,
    pub instantiated: Vec<String>,
    /// Functions and closures with a Blueprint of their own, by start byte.
    pub functions: HashMap<usize, SymbolId>,
}

/// The name of a method call (`x.foo::<T>(…)` → `foo`) and its receiver.
pub fn method_call<'t>(call: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    let f = call.child_by_field_name("function")?;
    let f = if f.kind() == "generic_function" { f.child_by_field_name("function")? } else { f };
    if f.kind() != "field_expression" {
        return None;
    }
    Some((f.child_by_field_name("value")?, f.child_by_field_name("field")?))
}

/// The written callee of a plain call (`thread::spawn`, `foo`), without generic arguments.
pub fn callee_path(src: Src, call: Node) -> Option<String> {
    let f = call.child_by_field_name("function")?;
    let f = if f.kind() == "generic_function" { f.child_by_field_name("function")? } else { f };
    match f.kind() {
        "identifier" | "scoped_identifier" | "self" | "crate" | "super" => Some(src.norm(f)),
        _ => None,
    }
}

/// The macro name of a `macro_invocation` (`println`, `vec`, `anyhow::bail` → `bail`).
pub fn macro_name(src: Src, m: Node) -> String {
    m.child_by_field_name("macro").map(|n| last_segment(&src.norm(n)).to_owned()).unwrap_or_default()
}

/// Splits a macro's token tree at top-level commas: `("{} {}", a, b.c)` → byte ranges of each part.
pub fn macro_args(src: Src, m: Node) -> Vec<(usize, usize)> {
    let Some(tt) = named_children(m).into_iter().find(|c| c.kind() == "token_tree") else { return Vec::new() };
    let text = src.text(tt);
    let base = tt.start_byte();
    let bytes = text.as_bytes();
    if bytes.len() < 2 {
        return Vec::new();
    }
    let (mut depth, mut start, mut out) = (0i32, 1usize, Vec::new());
    let mut in_str = false;
    let mut i = 1;
    while i < bytes.len() - 1 {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
            }
        } else {
            match b {
                b'"' => in_str = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' | b';' if depth == 0 => {
                    out.push((base + start, base + i));
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    out.push((base + start, base + bytes.len() - 1));
    out.into_iter()
        .map(|(s, e)| {
            let piece = &src.text[s..e];
            let lead = piece.len() - piece.trim_start().len();
            let trail = piece.len() - piece.trim_end().len();
            (s + lead, e - trail)
        })
        .filter(|(s, e)| e > s)
        .collect()
}

/// Names captured inside a format string: `"{total} {x:?} {}"` → `total`, `x`.
pub fn format_captures(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        rest = &rest[i + 1..];
        if rest.starts_with('{') {
            rest = &rest[1..];
            continue;
        }
        let end = rest.find('}').unwrap_or(rest.len());
        let inner = rest[..end].split(':').next().unwrap_or("").trim();
        if !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == '_') && !inner.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            let name = inner.to_owned();
            if !out.contains(&name) {
                out.push(name);
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    out
}

impl<'a> ExprLowering<'a> {
    pub fn new(src: Src<'a>) -> Self {
        Self { src, instantiated: Vec::new(), functions: HashMap::new() }
    }

    /// Detects an iterator pipeline rooted at `node`.
    pub fn pipeline<'t>(&self, node: Node<'t>) -> Option<Pipeline<'t>> {
        let mut stages = Vec::new();
        let mut cur = unparen(node);
        while cur.kind() == "call_expression" {
            let Some((recv, name)) = method_call(cur) else { break };
            stages.push((self.src.norm(name), cur));
            cur = unparen(recv);
        }
        // `x?` inside a chain (`fs::read(p)?.lines()`): the chain starts after it.
        stages.reverse();
        let is_op = |n: &str| ITER_OPS.contains(&n);
        let first = stages.iter().position(|(n, _)| is_op(n))?;
        let count = stages[first..].iter().filter(|(n, _)| is_op(n)).count();
        let has_source = stages[first..].iter().any(|(n, _)| SOURCES.contains(&n.as_str()));
        if count < 2 || (!has_source && count < 3) {
            return None;
        }
        let source = if first == 0 { cur } else { stages[first - 1].1 };
        Some(Pipeline { source, stages: stages.split_off(first) })
    }

    fn stage_label(&self, name: &str, call: Node) -> String {
        let args = call.child_by_field_name("arguments").map(|a| self.src.norm(a)).unwrap_or_else(|| "()".into());
        truncate(&format!("{name}{args}"), 48)
    }

    /// Natural one-line text for an expression; pipelines read as `source → filter(…) → sum()`.
    pub fn natural(&self, node: Node) -> String {
        let node = unparen(node);
        if let Some(p) = self.pipeline(node) {
            let mut parts = vec![truncate(&self.src.norm(p.source), 40)];
            parts.extend(p.stages.iter().map(|(n, c)| self.stage_label(n, *c)));
            return parts.join(" → ");
        }
        match node.kind() {
            "match_expression" => {
                let value = node.child_by_field_name("value").map(|v| self.natural(v)).unwrap_or_default();
                let arms = node.child_by_field_name("body").map(|b| named_children(b).iter().filter(|c| c.kind() == "match_arm").count()).unwrap_or(0);
                format!("match {value} ({arms} cases)")
            }
            "if_expression" => {
                let cond = node.child_by_field_name("condition").map(|c| self.natural(c)).unwrap_or_default();
                format!("if {cond} then … else …")
            }
            "block" | "unsafe_block" | "async_block" | "loop_expression" | "while_expression" | "for_expression" => {
                let t = self.src.norm(node);
                truncate(&t, 40)
            }
            "try_expression" => {
                let inner = named_children(node).into_iter().next().map(|i| self.natural(i)).unwrap_or_default();
                format!("{inner}?")
            }
            _ => self.src.norm(node),
        }
    }

    fn mk(&self, node: Node, concept: Concept, label: String, children: Vec<Expr>) -> Expr {
        Expr {
            syntax_sites: None,
            type_info: None,
            value_info: None,
            concept,
            label,
            focus: expression_focus(self.src, node, concept),
            span: self.src.span(node),
            flavors: Vec::new(),
            children,
            symbol: None,
            target: None,
            op: None,
            ty: None,
            params: Vec::new(),
            param_pins: None,
        }
    }

    pub fn lower(&mut self, node: Node) -> Expr {
        let src = self.src;
        let node = unparen(node);
        let text = || truncate(&src.norm(node), 40);
        let with = |mut e: Expr, flavor: &str| {
            if !e.flavors.iter().any(|f| f == flavor) {
                e.flavors.push(flavor.to_owned());
            }
            e
        };
        match node.kind() {
            "call_expression" => self.call(node),
            "macro_invocation" => self.macro_call(node),
            "field_expression" => {
                let field = node.child_by_field_name("field").map(|f| src.norm(f)).unwrap_or_default();
                let kids = node.child_by_field_name("value").map(|v| vec![self.lower(v)]).unwrap_or_default();
                self.mk(node, Concept::GetMember, format!(".{field}"), kids)
            }
            "scoped_identifier" => {
                // `OrderStatus::Paid`, `i32::MAX`, `std::f64::consts::PI`: a member of a path.
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let kids = node.child_by_field_name("path").map(|p| vec![self.mk(p, Concept::Var, src.norm(p), Vec::new())]).unwrap_or_default();
                self.mk(node, Concept::GetMember, format!(".{name}"), kids)
            }
            "index_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                self.mk(node, Concept::Index, "[ ]".into(), kids)
            }
            "binary_expression" => {
                let op = node.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_default();
                let kids: Vec<Expr> = ["left", "right"].iter().filter_map(|f| node.child_by_field_name(f)).map(|c| self.lower(c)).collect();
                let label = match op.as_str() {
                    "&&" => "and".to_owned(),
                    "||" => "or".to_owned(),
                    "!=" => "≠".to_owned(),
                    "<=" => "≤".to_owned(),
                    ">=" => "≥".to_owned(),
                    o => o.to_owned(),
                };
                let mut e = ops::operator(self.mk(node, Concept::BinaryOp, label, kids), &op, Equality::Strict);
                // `x == None`: a check for a missing value.
                if matches!(e.op.as_deref(), Some("equal" | "not_equal"))
                    && let Some(i) = e.children.iter().position(|c| c.label == "None")
                {
                    e.children.remove(i);
                    e.op = Some(if op == "!=" { "is_present".into() } else { "is_missing".into() });
                    e.ty = Some("bool".into());
                }
                e
            }
            "unary_expression" => {
                let op = children(node).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
                let inner = named_children(node).into_iter().next();
                if op == "*" {
                    return match inner {
                        Some(i) => with(self.lower(i), "deref"),
                        None => self.mk(node, Concept::Literal, text(), Vec::new()),
                    };
                }
                let kids = inner.map(|i| vec![self.lower(i)]).unwrap_or_default();
                let label = if op == "!" { "not".to_owned() } else { op.clone() };
                ops::operator(self.mk(node, Concept::UnaryOp, label, kids), &op, Equality::Strict)
            }
            "reference_expression" => {
                let inner = node.child_by_field_name("value");
                let tokens = children(node);
                let flavor = if tokens.iter().any(|c| c.kind() == "raw") { "raw-borrow" }
                    else if tokens.iter().any(|c| c.kind() == "mutable_specifier") { "borrow-mut" }
                    else { "borrow" };
                match inner {
                    Some(i) => with(self.lower(i), flavor),
                    None => self.mk(node, Concept::Literal, text(), Vec::new()),
                }
            }
            "try_expression" => match named_children(node).into_iter().next() {
                Some(i) => with(self.lower(i), "try"),
                None => self.mk(node, Concept::Opaque, text(), Vec::new()),
            },
            "await_expression" => {
                let kids = named_children(node).into_iter().take(1).map(|c| self.lower(c)).collect();
                self.mk(node, Concept::Await, "await".into(), kids)
            }
            "assignment_expression" => {
                if let Some((_, end)) = inclusive_range_to(node) {
                    let end = self.lower(end);
                    return with(self.mk(node, Concept::Compose, format!("range {}", text()), vec![end]), "range");
                }
                let kids = ["left", "right"].iter().filter_map(|f| node.child_by_field_name(f)).map(|c| self.lower(c)).collect();
                self.mk(node, Concept::Assign, "=".into(), kids)
            }
            "compound_assignment_expr" => {
                let op = node.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_default();
                let kids = ["left", "right"].iter().filter_map(|f| node.child_by_field_name(f)).map(|c| self.lower(c)).collect();
                let e = self.mk(node, Concept::Assign, op.clone(), kids);
                ops::compound_assign(e, &op, Equality::Strict)
            }
            "type_cast_expression" => {
                let ty = node.child_by_field_name("type").map(|t| src.norm(t)).unwrap_or_default();
                let kids = node.child_by_field_name("value").map(|v| vec![self.lower(v)]).unwrap_or_default();
                let mut e = self.mk(node, Concept::Convert, format!("as {ty}"), kids);
                e.ty = number_type(&ty).then(|| "number".to_owned());
                e
            }
            "if_expression" => {
                let mut kids = Vec::new();
                if let Some(c) = node.child_by_field_name("condition") {
                    kids.push(self.lower(c));
                }
                if let Some(t) = node.child_by_field_name("consequence") {
                    kids.push(self.lower(t));
                }
                if let Some(a) = node.child_by_field_name("alternative") {
                    let inner = named_children(a).into_iter().next().unwrap_or(a);
                    kids.push(self.lower(inner));
                }
                self.mk(node, Concept::Choose, "choose".into(), kids)
            }
            "let_condition" => {
                let pattern = node.child_by_field_name("pattern").map(|p| src.norm(p)).unwrap_or_default();
                let kids = node.child_by_field_name("value").map(|v| vec![self.lower(v)]).unwrap_or_default();
                with(self.mk(node, Concept::TypeCheck, truncate(&format!("matches {pattern}"), 40), kids), "if-let")
            }
            "let_chain" => {
                let kids: Vec<Expr> = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = self.mk(node, Concept::BinaryOp, "and".into(), kids);
                e.op = Some("and".into());
                e.concept = Concept::Operation;
                e.ty = Some("bool".into());
                e
            }
            "match_expression" => {
                let mut kids = Vec::new();
                if let Some(v) = node.child_by_field_name("value") {
                    kids.push(self.lower(v));
                }
                if let Some(body) = node.child_by_field_name("body") {
                    for arm in named_children(body).into_iter().filter(|a| a.kind() == "match_arm") {
                        let pattern = arm.child_by_field_name("pattern").map(|p| src.norm(p)).unwrap_or_default();
                        let value = arm.child_by_field_name("value").map(|v| vec![self.lower(v)]).unwrap_or_default();
                        let mut a = self.mk(arm, Concept::Match, truncate(&format!("{pattern} ⇒"), 40), value);
                        a.flavors.push("arm".into());
                        kids.push(a);
                    }
                }
                self.mk(node, Concept::Match, "match".into(), kids)
            }
            "block" | "unsafe_block" | "async_block" | "const_block" | "gen_block" | "try_block" => {
                let block = if node.kind() == "block" { node } else { named_children(node).into_iter().find(|c| c.kind() == "block").unwrap_or(node) };
                let tail = block_statements(block).next_back().and_then(statement_tail);
                let kids = tail.map(|t| vec![self.lower(t)]).unwrap_or_default();
                let label = match node.kind() {
                    "unsafe_block" => "unsafe block",
                    "async_block" => "async block",
                    "const_block" => "computed while compiling",
                    "gen_block" => "generator block",
                    "try_block" => "try block",
                    _ => "block value",
                };
                let mut e = self.mk(node, Concept::Compose, label.into(), kids);
                e.flavors.push(if node.kind() == "async_block" { "async" } else { "block" }.into());
                if node.kind() == "unsafe_block" {
                    e.flavors.push("unsafe".into());
                }
                e
            }
            "closure_expression" => self.closure(node),
            "struct_expression" => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                self.instantiated.push(crate::syntax::bare_type_name(&name));
                let mut kids = Vec::new();
                if let Some(body) = node.child_by_field_name("body") {
                    for f in named_children(body) {
                        match f.kind() {
                            "field_initializer" => {
                                let field = f.child_by_field_name("field").map(|n| src.norm(n)).unwrap_or_default();
                                if let Some(v) = f.child_by_field_name("value") {
                                    let mut e = self.lower(v);
                                    e.label = truncate(&format!("{field}: {}", e.label), 48);
                                    kids.push(e);
                                }
                            }
                            "shorthand_field_initializer" => kids.push(self.mk(f, Concept::Var, src.norm(f), Vec::new())),
                            "base_field_initializer" => {
                                let inner = named_children(f).into_iter().next();
                                let mut e = inner.map(|i| self.lower(i)).unwrap_or_else(|| self.mk(f, Concept::Literal, src.norm(f), Vec::new()));
                                e.flavors.push("spread".into());
                                kids.push(e);
                            }
                            _ => {}
                        }
                    }
                }
                self.mk(node, Concept::Construct, format!("new {}", truncate(&name, 32)), kids)
            }
            "tuple_expression" | "array_expression" => {
                let kids = named_children(node).into_iter().filter(|c| c.kind() != "attribute_item").map(|c| self.lower(c)).collect();
                let label = if node.kind() == "tuple_expression" {
                    "tuple"
                } else if nodex_lang_api::syntax_sites::rust_array_repeat(node).is_some() {
                    "repeated items"
                } else {
                    "list"
                };
                let mut e = self.mk(node, Concept::Compose, label.into(), kids);
                if node.kind() == "array_expression" {
                    e.ty = Some("list".into());
                }
                e
            }
            "range_expression" => {
                let kids = named_children(node).into_iter().map(|c| self.lower(c)).collect();
                let mut e = self.mk(node, Concept::Compose, format!("range {}", text()), kids);
                e.flavors.push("range".into());
                e
            }
            "return_expression" => {
                let kids = named_children(node).into_iter().take(1).map(|c| self.lower(c)).collect();
                self.mk(node, Concept::Return, "return".into(), kids)
            }
            "break_expression" | "continue_expression" => {
                let concept = if node.kind() == "break_expression" { Concept::Break } else { Concept::Continue };
                let kids = named_children(node).into_iter().filter(|c| c.kind() != "label").map(|c| self.lower(c)).collect();
                self.mk(node, concept, text(), kids)
            }
            "yield_expression" => {
                let kids = named_children(node).into_iter().take(1).map(|c| self.lower(c)).collect();
                self.mk(node, Concept::Yield, "yield".into(), kids)
            }
            "loop_expression" | "while_expression" | "for_expression" => {
                let mut e = self.mk(node, Concept::Compose, text(), Vec::new());
                e.flavors.push("loop-value".into());
                e
            }
            "string_literal" | "raw_string_literal" | "char_literal" | "integer_literal" | "float_literal" | "boolean_literal"
            | "negative_literal" | "unit_expression" => {
                let mut e = self.mk(node, Concept::Literal, text(), Vec::new());
                e.ty = match node.kind() {
                    "string_literal" | "raw_string_literal" | "char_literal" => Some("text".into()),
                    "integer_literal" | "float_literal" | "negative_literal" => Some("number".into()),
                    "boolean_literal" => Some("bool".into()),
                    _ => None,
                };
                e
            }
            "identifier" | "self" | "crate" | "super" | "metavariable" => self.mk(node, Concept::Var, text(), Vec::new()),
            "generic_function" => self.mk(node, Concept::Var, text(), Vec::new()),
            "_" | "function_modifiers" | "visibility_modifier" | "mutable_specifier" => self.mk(node, Concept::Literal, text(), Vec::new()),
            k if k.ends_with("_type") || k == "type_identifier" => self.mk(node, Concept::Var, text(), Vec::new()),
            _ => {
                let mut e = self.mk(node, Concept::Opaque, text(), Vec::new());
                e.flavors.push(format!("syntax:{}", node.kind()));
                e
            }
        }
    }

    fn closure(&mut self, node: Node) -> Expr {
        let src = self.src;
        let params: Vec<String> = parameters(src, node).into_iter().map(|(n, _)| n).collect();
        let body = node.child_by_field_name("body");
        let symbol = self.functions.get(&node.start_byte()).cloned();
        let kids = match body {
            Some(b) if symbol.is_some() && crate::structure::direct_loop_body(b) => Vec::new(),
            Some(b) if b.kind() != "block" => vec![self.lower(b)],
            Some(b) => {
                // `|x| { x + 1 }`: a one-expression block reads like an expression body.
                let mut inner = block_statements(b);
                match (inner.next(), inner.next()) {
                    (Some(only), None) => statement_tail(only).map(|tail| self.lower(tail)).into_iter().collect(),
                    _ => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        let mut e = self.mk(node, Concept::Lambda, truncate(&format!("|{}| …", params.join(", ")), 40), kids);
        e.params = params;
        e.symbol = symbol;
        let head = src.text(node).trim_start();
        if head.starts_with("move") || head.starts_with("async move") {
            e.flavors.push("move".into());
        }
        if head.starts_with("async") {
            e.flavors.push("async".into());
        }
        e
    }

    fn call(&mut self, node: Node) -> Expr {
        let src = self.src;
        if let Some(p) = self.pipeline(node) {
            let mut kids = vec![self.lower(p.source)];
            for (name, call) in &p.stages {
                let args = self.arguments(*call);
                let mut stage = self.mk(*call, Concept::Call, self.stage_label(name, *call), args);
                stage.flavors.push("stage".into());
                if !ITER_OPS.contains(&name.as_str()) {
                    stage.flavors.push("custom-stage".into());
                }
                kids.push(stage);
            }
            let mut e = self.mk(node, Concept::Pipeline, "pipeline".into(), kids);
            e.flavors.push("iterator".into());
            return e;
        }
        let mut kids = Vec::new();
        let mut flavors = Vec::new();
        let label = if let Some((recv, name)) = method_call(node) {
            let mut target = self.lower(recv);
            target.flavors.push("target".into());
            kids.push(target);
            let name = src.norm(name);
            if let Some(f) = node.child_by_field_name("function").filter(|f| f.kind() == "generic_function")
                && let Some(args) = f.child_by_field_name("type_arguments")
            {
                flavors.push(format!("turbofish:{}", src.norm(args).trim_matches(['<', '>'])));
            }
            name
        } else {
            let path = callee_path(src, node).unwrap_or_else(|| node.child_by_field_name("function").map(|f| src.norm(f)).unwrap_or_default());
            // `Order::new(..)`, `Vec::with_capacity(..)`: a constructor by convention.
            let seg: Vec<&str> = path.split("::").collect();
            if seg.len() >= 2 {
                let ty = seg[seg.len() - 2];
                let ctor = seg[seg.len() - 1];
                if ty.chars().next().is_some_and(char::is_uppercase) && matches!(ctor, "new" | "with_capacity" | "default" | "from" | "builder")
                    && !matches!(ty, "Some" | "Ok" | "Err")
                {
                    self.instantiated.push(crate::syntax::bare_type_name(ty));
                    let args = self.arguments(node);
                    let mut e = self.mk(node, Concept::Construct, format!("new {}", truncate(ty, 32)), args);
                    if ctor != "new" {
                        e.flavors.push(format!("ctor:{ctor}"));
                    }
                    return e;
                }
            }
            if let Some(f) = node.child_by_field_name("function")
                && f.kind() == "field_expression"
            {
                // `(self.callback)(x)`: calling a function stored in a field.
                flavors.push("function-value".into());
            }
            truncate(&path, 40)
        };
        kids.extend(self.arguments(node));
        let mut e = self.mk(node, Concept::Call, label, kids);
        e.flavors = flavors;
        // `parse::<i32>()` reads a whole number; `parse()` alone a number.
        if e.label == "parse" && e.flavors.iter().any(|f| f.starts_with("turbofish:") && is_integer_type(f.trim_start_matches("turbofish:"))) {
            e.op = Some("convert.to_integer".into());
            e.ty = Some("number".into());
        }
        // Enum variant constructors: `Some(x)`, `Ok(v)`, `Shape::Circle(r)`.
        if matches!(e.label.as_str(), "Some" | "Ok" | "Err") || last_segment(&e.label).chars().next().is_some_and(char::is_uppercase) {
            e.concept = Concept::Construct;
            e.flavors.push("variant".into());
        }
        e
    }

    fn arguments(&mut self, call: Node) -> Vec<Expr> {
        call.child_by_field_name("arguments")
            .map(|a| named_children(a).into_iter().filter(|c| c.kind() != "attribute_item").map(|c| self.lower(c)).collect())
            .unwrap_or_default()
    }

    /// `println!("{} {}", a, b)`: a call to the macro with its arguments parsed as expressions.
    fn macro_call(&mut self, node: Node) -> Expr {
        let src = self.src;
        let name = macro_name(src, node);
        let mut kids = Vec::new();
        let args = macro_args(src, node);
        if !matches!(name.as_str(), "macro_rules") {
            for (s, e) in &args {
                kids.push(self.piece(*s, *e));
            }
        }
        // Names captured by the format string (`println!("{total}")`).
        if FORMAT_MACROS.contains(&name.as_str())
            && let Some((s, e)) = args.first()
        {
            for cap in format_captures(&src.text[*s..*e]) {
                if !kids.iter().any(|k| k.concept == Concept::Var && k.label == cap) {
                    let mut v = self.mk(node, Concept::Var, cap, Vec::new());
                    v.focus = Src::focus_text(&v.label);
                    v.flavors.push("captured".into());
                    kids.push(v);
                }
            }
        }
        let mut e = self.mk(node, Concept::Call, format!("{name}!"), kids);
        e.flavors.push("macro".into());
        if name == "vec" {
            e.ty = Some("list".into());
        }
        if name == "format" {
            e.ty = Some("text".into());
        }
        e
    }

    /// One macro argument, parsed on its own (spans stay those of the file).
    fn piece(&mut self, start: usize, end: usize) -> Expr {
        let src = self.src;
        let text = &src.text[start..end];
        let mut parser = tree_sitter::Parser::new();
        let leaf = |concept: Concept, ty: Option<&str>| Expr {
            syntax_sites: None,
            type_info: None,
            value_info: None,
            concept,
            label: truncate(&nodex_lang_api::kit::normalize_ws(text), 40),
            focus: None,
            span: src.index.span(src.file, start, end),
            flavors: Vec::new(),
            children: Vec::new(),
            symbol: None,
            target: None,
            op: None,
            ty: ty.map(str::to_owned),
            params: Vec::new(),
            param_pins: None,
        };
        if text.starts_with('"') || text.starts_with("r#") || text.starts_with("r\"") {
            return leaf(Concept::Literal, Some("text"));
        }
        if parser.set_language(&crate::grammar()).is_err() {
            return leaf(Concept::Compose, None);
        }
        let range = tree_sitter::Range {
            start_byte: start,
            end_byte: end,
            start_point: point_at(src.text, start),
            end_point: point_at(src.text, end),
        };
        if parser.set_included_ranges(&[range]).is_err() {
            return leaf(Concept::Compose, None);
        }
        let Some(tree) = parser.parse(src.text, None) else { return leaf(Concept::Compose, None) };
        let root = tree.root_node();
        let expr = named_children(root).into_iter().next().map(|n| if n.kind() == "expression_statement" { named_children(n).into_iter().next().unwrap_or(n) } else { n });
        match expr {
            Some(n) if !n.has_error() && n.start_byte() == start && is_expression(n.kind()) => self.lower(n),
            _ => leaf(Concept::Compose, None),
        }
    }
}

fn point_at(text: &str, byte: usize) -> tree_sitter::Point {
    let before = &text[..byte];
    let row = before.matches('\n').count();
    let col = byte - before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    tree_sitter::Point { row, column: col }
}

fn number_type(ty: &str) -> bool {
    matches!(ty, "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64")
}

fn is_integer_type(ty: &str) -> bool {
    number_type(ty) && !ty.starts_with('f')
}

/// A read of a local variable and how its value is handed over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    pub name: String,
    pub access: Option<Access>,
    projected: bool,
}

/// Locals read by `node` (names that may refer to locals or parameters) with their access.
/// Closure parameters shadow; paths, fields and macro names are not reads.
pub fn reads(src: Src, node: Node) -> Vec<Read> {
    let mut out = Vec::new();
    collect(src, node, &mut Vec::new(), &mut out);
    out
}

fn push(out: &mut Vec<Read>, name: String, access: Option<Access>) {
    push_place(out, name, access, false);
}

fn push_place(out: &mut Vec<Read>, name: String, access: Option<Access>, projected: bool) {
    let rank = |a: Option<Access>| match a {
        Some(Access::BorrowMut) => 4,
        Some(Access::Move | Access::ByValue) => 3,
        Some(Access::Borrow) => 2,
        Some(Access::Copy) => 1,
        None => 0,
    };
    match out.iter_mut().find(|r| r.name == name) {
        Some(r) => {
            if rank(access) > rank(r.access) {
                r.access = access;
            }
            r.projected &= projected;
        }
        None => out.push(Read { name, access, projected }),
    }
}

/// How the value of identifier `id` is used by its parent.
fn access_of(src: Src, id: Node) -> Option<Access> {
    let parent = id.parent()?;
    let is_field = |f: &str| parent.child_by_field_name(f).is_some_and(|c| c.id() == id.id());
    match parent.kind() {
        "reference_expression" => {
            let tokens = children(parent);
            if tokens.iter().any(|c| c.kind() == "raw") { return None; }
            Some(if tokens.iter().any(|c| c.kind() == "mutable_specifier") { Access::BorrowMut } else { Access::Borrow })
        }
        // `x.len()`, `x.field`, `x[i]`, `*x`, operators: the language borrows or copies on its own.
        "field_expression" | "index_expression" | "unary_expression" | "binary_expression" | "compound_assignment_expr"
        | "range_expression" | "type_cast_expression" | "try_expression" | "await_expression" | "if_expression"
        | "while_expression" | "match_expression" | "let_condition" => None,
        "assignment_expression" if is_field("left") || inclusive_range_to(parent).is_some() => None,
        "let_declaration" | "arguments" | "return_expression" | "field_initializer" | "shorthand_field_initializer"
        | "array_expression" | "tuple_expression" | "assignment_expression" | "break_expression" | "closure_expression"
        | "block" => Some(Access::Move),
        "for_expression" if is_field("value") => Some(Access::Move),
        _ => {
            let _ = src;
            None
        }
    }
}

fn collect(src: Src, node: Node, shadowed: &mut Vec<String>, out: &mut Vec<Read>) {
    if crate::syntax::discarded_path(node) {
        return;
    }
    match node.kind() {
        "identifier" => {
            let name = src.text(node).to_owned();
            if !shadowed.contains(&name) && !is_non_value(node) {
                let projected = crate::syntax::field_projection(node).is_some();
                push_place(out, name, access_of(src, node), projected);
            }
            return;
        }
        "self" => {
            push(out, "self".into(), access_of(src, node));
            return;
        }
        "closure_expression" => {
            let before = shadowed.len();
            for (n, _) in parameters(src, node) {
                shadowed.push(n);
            }
            let moves = src.text(node).trim_start().starts_with("move") || src.text(node).trim_start().starts_with("async move");
            if let Some(body) = node.child_by_field_name("body") {
                let mut inner = Vec::new();
                collect(src, body, shadowed, &mut inner);
                for r in inner {
                    let access = if moves && r.projected { None }
                        else if moves { Some(Access::Move) }
                        else { r.access.filter(|a| *a == Access::BorrowMut).or(Some(Access::Borrow)) };
                    push_place(out, r.name, access, r.projected);
                }
            }
            shadowed.truncate(before);
            return;
        }
        "let_chain" => {
            let before = shadowed.len();
            for condition in named_children(node) {
                collect(src, condition, shadowed, out);
                if condition.kind() == "let_condition"
                    && let Some(pattern) = condition.child_by_field_name("pattern")
                {
                    let mutable = first_child_of_kind(condition, "mutable_specifier").is_some();
                    let mut bindings = Vec::new();
                    pattern_bindings(src, pattern, mutable, &mut bindings);
                    shadowed.extend(bindings.into_iter().map(|(name, _)| name));
                }
            }
            shadowed.truncate(before);
            return;
        }
        "macro_invocation" => {
            let name = macro_name(src, node);
            let borrows = FORMAT_MACROS.contains(&name.as_str());
            let args = macro_args(src, node);
            if let Some(tt) = named_children(node).into_iter().find(|c| c.kind() == "token_tree") {
                let mut ids = Vec::new();
                token_identifiers(tt, &mut ids);
                for id in ids {
                    let name = src.text(id).to_owned();
                    let next = src.text.get(id.end_byte()..).unwrap_or("").trim_start();
                    let prev = src.text.get(..id.start_byte()).unwrap_or("").trim_end();
                    // `a.b` (field), `f(` (function), `A::b` (path), `x!` (macro) are not plain reads.
                    if next.starts_with("::") || next.starts_with('!') || prev.ends_with('.') || prev.ends_with("::") || shadowed.contains(&name) {
                        continue;
                    }
                    let reference = prev.ends_with('&') || prev.ends_with("&mut");
                    let access = if prev.ends_with("&mut") {
                        Some(Access::BorrowMut)
                    } else if reference || borrows {
                        Some(Access::Borrow)
                    } else {
                        None
                    };
                    push(out, name, access);
                }
            }
            if borrows && let Some((s, e)) = args.first() {
                for cap in format_captures(&src.text[*s..*e]) {
                    if !shadowed.contains(&cap) {
                        push(out, cap, Some(Access::Borrow));
                    }
                }
            }
            return;
        }
        "scoped_identifier" | "scoped_type_identifier" | "type_identifier" | "field_identifier" | "type_arguments" | "lifetime" | "label"
        | "attribute_item" | "line_comment" | "block_comment" => return,
        "struct_expression" => {
            if let Some(body) = node.child_by_field_name("body") {
                collect(src, body, shadowed, out);
            }
            return;
        }
        "shorthand_field_initializer" => {
            if let Some(id) = named_children(node).into_iter().next() {
                let name = src.text(id).to_owned();
                if !shadowed.contains(&name) {
                    push(out, name, Some(Access::Move));
                }
            }
            return;
        }
        "call_expression" => {
            // The callee of `f(x)` is read only when it is a local closure; its arguments are moved.
            if let Some(f) = node.child_by_field_name("function") {
                if f.kind() == "identifier" {
                    let name = src.text(f).to_owned();
                    if !shadowed.contains(&name) {
                        push(out, name, None);
                    }
                } else {
                    collect(src, f, shadowed, out);
                }
            }
            if let Some(a) = node.child_by_field_name("arguments") {
                collect(src, a, shadowed, out);
            }
            return;
        }
        _ => {}
    }
    for c in named_children(node) {
        collect(src, c, shadowed, out);
    }
}

fn token_identifiers<'t>(tt: Node<'t>, out: &mut Vec<Node<'t>>) {
    for c in named_children(tt) {
        match c.kind() {
            "identifier" => out.push(c),
            "token_tree" => token_identifiers(c, out),
            _ => {}
        }
    }
}

/// Identifiers that name something other than a local value.
fn is_non_value(node: Node) -> bool {
    let Some(parent) = node.parent() else { return false };
    let field_is = |f: &str| parent.child_by_field_name(f).is_some_and(|c| c.id() == node.id());
    match parent.kind() {
        "field_expression" => field_is("field"),
        "macro_invocation" => field_is("macro"),
        "generic_function" => field_is("function"),
        "field_initializer" => field_is("field"),
        "let_declaration" | "parameter" | "for_expression" | "let_condition" => field_is("pattern"),
        "tuple_pattern" | "slice_pattern" | "tuple_struct_pattern" | "struct_pattern" | "field_pattern" | "mut_pattern" | "ref_pattern"
        | "captured_pattern" | "or_pattern" | "match_pattern" | "closure_parameters" | "reference_pattern" => true,
        "struct_expression" => field_is("name"),
        "break_expression" | "continue_expression" => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_strings_capture_names() {
        assert_eq!(format_captures("Total: {total} {x:?} {} {{literal}} {0}"), ["total", "x"]);
    }
}
