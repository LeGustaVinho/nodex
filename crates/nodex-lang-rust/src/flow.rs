//! Function bodies → Blueprint flow graphs.
//!
//! Same conventions as every language (see the "Visual Language" doc): one impure node per
//! statement with its expression tree kept for on-demand expansion; compound statements own
//! named exec outputs (`true`, `body`, `case:N`) and resume at `out`. Rust specifics:
//! - a function's tail expression is its `Return` (also inside the arms of a tail `if`/`match`);
//! - `expr?` adds an `error` exec output: if it fails, the error is returned right there;
//! - data wires say whether the value moves, is copied or is borrowed.

use std::collections::{HashMap, HashSet};

use nodex_ir::{Access, CallTarget, Concept, FlowEdgeKind, FlowGraph, Language, SymbolId, TypeRef, TypeRelation};
use nodex_lang_api::kit::{ExecCursor, FlowBuilder, NodeSpec, callback_entry_label, truncate};
use tree_sitter::Node;

use crate::expr::{ExprLowering, callee_path, is_expression, macro_args, macro_name, method_call, reads};
use crate::structure::{BodyJob, BodyKind};
use crate::syntax::{Src, children, first_child_of_kind, has_semicolon, is_comment, is_item, named_children, parameters, pattern_names, unparen};

pub struct Lowered {
    pub graph: FlowGraph,
    pub type_refs: Vec<TypeRef>,
}

type Flow = Option<ExecCursor>;

struct Lowering<'a, 'f> {
    src: Src<'a>,
    b: FlowBuilder,
    ex: ExprLowering<'a>,
    functions: &'f HashMap<usize, SymbolId>,
    label_max: usize,
    /// How many times each local name was defined (shadowing: `value`, `value₂`, ...).
    defined: HashMap<String, u32>,
    /// Locals whose end of scope runs code (`Drop`), per open block.
    drops: Vec<Vec<DropLocal>>,
    /// Project types with `impl Drop` → their `drop` method.
    drop_types: &'f HashMap<String, SymbolId>,
    /// While lowering `let x = loop { … }`: `break value` sets `x`.
    target: Option<String>,
}

/// A local that runs code when its scope ends.
struct DropLocal {
    name: String,
    ty: String,
    /// The node and pin that define it.
    node: String,
    pin: String,
}

/// What a block's final expression (without `;`) does.
#[derive(Clone, PartialEq, Eq)]
enum Tail {
    /// Nothing: its value is discarded.
    No,
    /// It is the function's result.
    Return,
    /// It is the value of `let name = …`.
    Set(String),
}

impl Tail {
    fn on(&self) -> bool {
        *self != Tail::No
    }
}

/// `2` → `₂`.
fn subscript(n: u32) -> String {
    n.to_string().chars().map(|c| char::from_u32(0x2080 + c.to_digit(10).unwrap_or(0)).unwrap_or(c)).collect()
}

/// Guards that release something when they go out of scope.
fn is_guard(ty: &str) -> bool {
    ["MutexGuard", "RwLockReadGuard", "RwLockWriteGuard", "RefMut", "Ref<"].iter().any(|g| ty.starts_with(g))
}

/// Trait methods Rust calls on its own: what triggers them.
fn trait_event(trait_name: &str, method: &str, ty: &str) -> Option<String> {
    Some(match (trait_name, method) {
        ("Drop", "drop") => format!("Rust calls this when a {ty} value goes out of scope or is dropped"),
        ("Display", "fmt") => format!("when a {ty} is formatted as text ({{}} in println!, format!, to_string)"),
        ("Debug", "fmt") => format!("when a {ty} is formatted for debugging ({{:?}})"),
        ("Iterator", "next") => "when the next item is requested (a for loop asks for one each time)".to_owned(),
        ("IntoIterator", "into_iter") => format!("when a {ty} is used in a for loop"),
        ("From", "from") => format!("when a value is converted into a {ty} (From / .into())"),
        ("TryFrom", "try_from") => format!("when a value is converted into a {ty} and may fail (try_into)"),
        ("FromStr", "from_str") => format!("when text is parsed into a {ty} (.parse())"),
        ("Default", "default") => format!("when a default {ty} is needed (Default::default(), ..Default::default())"),
        ("Clone", "clone") => format!("when a {ty} is copied with .clone()"),
        ("PartialEq", "eq") => format!("when two {ty} values are compared with == or !="),
        ("PartialOrd", "partial_cmp") | ("Ord", "cmp") => format!("when {ty} values are compared or sorted"),
        ("Hash", "hash") => format!("when a {ty} is used as a key (HashMap, HashSet)"),
        ("Deref", "deref") => format!("when a {ty} is used through * or a method of what it wraps"),
        ("DerefMut", "deref_mut") => format!("when what a {ty} wraps is changed through it"),
        ("Index", "index") => format!("when a {ty} is read with [ ]"),
        ("IndexMut", "index_mut") => format!("when a {ty} is written with [ ]"),
        ("Add", "add") => format!("when + is used on a {ty}"),
        ("Sub", "sub") => format!("when - is used on a {ty}"),
        ("Mul", "mul") => format!("when * is used on a {ty}"),
        ("Div", "div") => format!("when / is used on a {ty}"),
        ("Neg", "neg") => format!("when a {ty} is negated (-x)"),
        ("AddAssign", "add_assign") => format!("when += is used on a {ty}"),
        ("Future", "poll") => format!("when the async runtime checks whether the {ty} is ready"),
        ("Error", "source") => format!("when the cause of a {ty} error is asked for"),
        ("AsRef", "as_ref") | ("Borrow", "borrow") => format!("when a {ty} is used as a reference to what it holds"),
        _ => return None,
    })
}


const ENTRY_FLAVORS: &[&str] = &[
    "async", "const", "unsafe", "extern", "&self", "&mut self", "self", "static", "test", "generic", "default", "local-function",
    "callback", "move", "pub", "async-main",
];

/// Calls that run their argument on another thread or task.
const SPAWNS: &[(&str, &str)] = &[
    ("thread::spawn", "on another thread"),
    ("std::thread::spawn", "on another thread"),
    ("spawn", "on another thread"),
    ("tokio::spawn", "as a background task"),
    ("tokio::task::spawn", "as a background task"),
    ("task::spawn", "as a background task"),
    ("tokio::task::spawn_blocking", "on a blocking thread"),
    ("spawn_blocking", "on a blocking thread"),
    ("rayon::spawn", "on the thread pool"),
    ("async_std::task::spawn", "as a background task"),
];

const SLEEPS: &[&str] = &["thread::sleep", "std::thread::sleep", "sleep", "tokio::time::sleep", "time::sleep", "async_std::task::sleep"];

/// Macros that stop the program (panic).
const PANICS: &[&str] = &["panic", "unreachable", "todo", "unimplemented"];

/// Macros whose value is `()`: a tail call to one is a statement, not a result.
const UNIT_MACROS: &[&str] = &[
    "println", "print", "eprintln", "eprint", "assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq",
    "debug_assert_ne", "info", "warn", "error", "debug", "trace",
];

/// `Duration::from_millis(10)` → `10 milliseconds`.
fn duration_text(src: Src, e: Node) -> Option<String> {
    let e = unparen(e);
    if e.kind() != "call_expression" {
        return None;
    }
    let path = callee_path(src, e)?;
    let unit = match crate::syntax::last_segment(&path) {
        "from_millis" => "milliseconds",
        "from_secs" | "from_secs_f64" | "from_secs_f32" => "seconds",
        "from_micros" => "microseconds",
        "from_nanos" => "nanoseconds",
        _ => return None,
    };
    let arg = e.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next())?;
    Some(format!("{} {unit}", src.norm(arg)))
}

/// Macros of the standard library (and `log`): shown as calls; others are `Macro` nodes.
const KNOWN_MACROS: &[&str] = &[
    "println", "print", "eprintln", "eprint", "format", "write", "writeln", "vec", "assert", "assert_eq", "assert_ne",
    "debug_assert", "debug_assert_eq", "debug_assert_ne", "dbg", "info", "warn", "error", "debug", "trace", "matches",
    "format_args", "env", "concat", "stringify", "include_str", "include_bytes", "file", "line", "column", "cfg",
];

pub fn lower_body(
    src: Src,
    job: &BodyJob,
    functions: &HashMap<usize, SymbolId>,
    copy_types: &HashSet<String>,
    drop_types: &HashMap<String, SymbolId>,
) -> Lowered {
    let mut l = Lowering {
        src,
        b: FlowBuilder::new(job.symbol.clone()),
        ex: ExprLowering { functions: functions.clone(), ..ExprLowering::new(src) },
        functions,
        label_max: 96,
        defined: HashMap::new(),
        drops: Vec::new(),
        drop_types,
        target: None,
    };
    let params: Vec<&str> = job.params.iter().map(|(n, _)| n.as_str()).collect();
    let label = match job.kind {
        BodyKind::LocalFunction if job.flavors.iter().any(|f| f == "callback") => callback_entry_label(&job.name, &params),
        BodyKind::LocalFunction => format!("when local function {}({}) is called", job.name, params.join(", ")),
        _ if job.flavors.iter().any(|f| f == "test") => format!("when the test {} runs", job.name),
        _ if job.name == "main" && job.kind == BodyKind::Function => "when the program starts".to_owned(),
        _ => format!("when {}({}) is called", job.name, params.join(", ")),
    };
    let entry_span = src.header_span(job.decl, Some(job.body));
    let flavors = job
        .flavors
        .iter()
        .filter(|f| ENTRY_FLAVORS.contains(&f.as_str()) || f.starts_with("from-trait:") || f.starts_with("attr:"))
        .cloned();
    // `impl Drop for Guard { fn drop … }`: Rust calls it, the code never does.
    let owner_type = job.symbol.rsplit("::").nth(1).unwrap_or("").to_owned();
    let event = job
        .flavors
        .iter()
        .find_map(|f| f.strip_prefix("from-trait:"))
        .and_then(|t| trait_event(t, job.name.split('~').next().unwrap_or(&job.name), &owner_type));
    let (concept, label) = match event {
        Some(e) => (Concept::EventEntry, e),
        None => (Concept::Entry, label),
    };
    let entry = l.b.add(NodeSpec::new(concept, label, job.signature.clone(), entry_span).flavors(flavors));
    let start = l.b.exec_out(&entry, "out", "");
    for (name, ty) in &job.params {
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            l.b.define(&entry, name, ty.clone());
        } else {
            let mut names = Vec::new();
            if let Some(list) = job.decl.child_by_field_name("parameters") {
                for p in named_children(list) {
                    if let Some(pat) = p.child_by_field_name("pattern") {
                        pattern_names(src, pat, &mut names);
                    }
                }
            }
            for n in names {
                l.b.define(&entry, &n, None);
            }
        }
    }

    if job.body.kind() == "block" {
        l.block(job.body, Some(start), &if job.returns_value { Tail::Return } else { Tail::No });
    } else if job.returns_value || job.kind == BodyKind::LocalFunction {
        l.return_value(job.body, Some(job.body), Some(start));
    } else {
        l.expr_stmt(job.body, job.body, Some(start));
    }

    let type_refs = l
        .ex
        .instantiated
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| TypeRef { from: job.symbol.clone(), name: n.clone(), relation: TypeRelation::Instantiates })
        .collect();
    let mut graph = l.b.finish(job.name.clone(), Language::Rust, src.span(job.decl));
    copies(&mut graph, copy_types);
    Lowered { graph, type_refs }
}

/// A move of a `Copy` value is a copy.
fn copies(graph: &mut FlowGraph, copy_types: &HashSet<String>) {
    let types: HashMap<(String, String), Option<String>> = graph
        .nodes
        .iter()
        .flat_map(|n| n.outputs.iter().map(move |p| ((n.id.clone(), p.id.clone()), p.ty.clone())))
        .collect();
    for e in graph.edges.iter_mut().filter(|e| e.kind == FlowEdgeKind::Data && e.access == Some(Access::Move)) {
        if let Some(Some(ty)) = types.get(&(e.from.clone(), e.from_pin.clone()))
            && is_copy(ty, copy_types)
        {
            e.access = Some(Access::Copy);
        }
    }
}

/// Primitive numbers, `bool`, `char`, shared references and project `Copy` types.
pub fn is_copy(ty: &str, copy_types: &HashSet<String>) -> bool {
    let t = ty.trim();
    if t.starts_with("&mut") {
        return false;
    }
    if t.starts_with('&') || t.starts_with("*const") || t.starts_with("fn(") {
        return true;
    }
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        return inner.split(',').map(str::trim).filter(|s| !s.is_empty()).all(|p| is_copy(p, copy_types));
    }
    matches!(
        t,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" | "bool" | "char" | "()"
            | "{integer}" | "{float}"
    ) || copy_types.contains(&crate::syntax::bare_type_name(t))
}

/// The type a literal gives a fresh variable (`0` → `i32`), so pins and copies are known.
fn literal_type(src: Src, value: Node) -> Option<String> {
    let v = unparen(value);
    Some(
        match v.kind() {
            "integer_literal" => {
                let t = src.text(v);
                ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize"]
                    .iter()
                    .find(|s| t.ends_with(*s))
                    .map(|s| (*s).to_owned())
                    .unwrap_or_else(|| "{integer}".into())
            }
            "float_literal" => {
                let t = src.text(v);
                if t.ends_with("f32") {
                    "f32".into()
                } else if t.ends_with("f64") {
                    "f64".into()
                } else {
                    "{float}".into()
                }
            }
            "boolean_literal" => "bool".into(),
            "char_literal" => "char".into(),
            "string_literal" | "raw_string_literal" => "&str".into(),
            "negative_literal" => "{integer}".into(),
            _ => return None,
        },
    )
}

/// True if `expr` (outside nested closures) carries `flavor`.
fn has_flavor(expr: &nodex_ir::Expr, flavor: &str) -> bool {
    expr.flavors.iter().any(|f| f == flavor) || (expr.concept != Concept::Lambda && expr.children.iter().any(|c| has_flavor(c, flavor)))
}

/// True if `node` uses `?` outside nested closures and blocks of its own.
fn has_try(node: Node) -> bool {
    if node.kind() == "try_expression" {
        return true;
    }
    named_children(node)
        .into_iter()
        .any(|c| !matches!(c.kind(), "closure_expression" | "block" | "async_block" | "function_item" | "try_block") && has_try(c))
}

/// A value that is control flow with statements of its own (`if`/`match` with blocks that do
/// more than give a value, `loop`, a labelled block).
fn has_inner_statements(v: Node) -> bool {
    let v = unparen(v);
    let busy_block = |b: Node| named_children(b).iter().filter(|c| !is_comment(c.kind()) && c.kind() != "label").count() > 1;
    match v.kind() {
        "loop_expression" | "while_expression" | "for_expression" => true,
        "block" => busy_block(v) || first_child_of_kind(v, "label").is_some(),
        "unsafe_block" => first_child_of_kind(v, "block").is_some_and(busy_block),
        "if_expression" => {
            let cons = v.child_by_field_name("consequence").is_some_and(busy_block);
            let alt = v.child_by_field_name("alternative").and_then(|a| named_children(a).into_iter().next()).is_some_and(|a| {
                if a.kind() == "block" { busy_block(a) } else { has_inner_statements(a) }
            });
            v.child_by_field_name("alternative").is_some() && (cons || alt)
        }
        "match_expression" => v.child_by_field_name("body").is_some_and(|b| {
            named_children(b)
                .into_iter()
                .filter(|a| a.kind() == "match_arm")
                .any(|a| a.child_by_field_name("value").is_some_and(|x| x.kind() == "block" && busy_block(x)))
        }),
        _ => false,
    }
}

/// Pattern of a `let` that binds just one name (`x`, `mut x`).
fn single_name(src: Src, pattern: Node) -> Option<String> {
    match pattern.kind() {
        "identifier" => Some(src.text(pattern).to_owned()),
        "mut_pattern" => named_children(pattern).into_iter().last().and_then(|p| single_name(src, p)),
        _ => None,
    }
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
        // What the statement's expression does that matters to the reader.
        for f in ["clone", "may-panic", "lock", "runtime-borrow"] {
            if spec.expr.as_ref().is_some_and(|e| has_flavor(e, f)) {
                spec = spec.flavor(f);
            }
        }
        let id = self.b.add(spec);
        if let Some(c) = cur {
            self.b.link(c, &id);
        }
        id
    }

    fn read_all(&mut self, id: &String, node: Option<Node>) {
        if let Some(n) = node {
            for r in reads(self.src, n) {
                self.b.read_with(id, &r.name, r.access);
            }
        }
    }

    /// `x?` in the statement: an extra exec output for the early return of the error.
    fn propagate(&mut self, id: &String, node: Option<Node>) {
        if node.is_some_and(has_try) {
            self.b.exec_out(id, "error", "If it fails (Err or None): return it right away");
            if let Some(i) = self.b.index_of(id) {
                self.b.add_flavor_at(i, "try");
            }
        }
    }

    fn simple(&mut self, s: Node, concept: Concept, label: String, expr: Option<Node>, flavors: Vec<String>, cur: Flow) -> Flow {
        let lowered = expr.map(|e| self.ex.lower(e));
        let spec = NodeSpec::new(concept, self.label(label), self.src.norm(s), self.src.span(s)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, expr);
        self.propagate(&id, expr);
        if concept.is_terminal() { None } else { Some(self.b.exec_out(&id, "out", "")) }
    }

    /// A block's statements; `tail` says what its final expression (without `;`) does.
    fn block(&mut self, block: Node, cur: Flow, tail: &Tail) -> Flow {
        let items: Vec<Node> = named_children(block).into_iter().filter(|c| !is_comment(c.kind()) && c.kind() != "label").collect();
        self.b.push_scope();
        self.drops.push(Vec::new());
        let mut cur = cur;
        let count = items.len();
        let mut ended_by_tail = false;
        for (i, s) in items.into_iter().enumerate() {
            let last = i + 1 == count;
            let tail_expr = match s.kind() {
                "expression_statement" if !has_semicolon(self.src, s) => named_children(s).into_iter().next(),
                k if is_expression(k) => Some(s),
                _ => None,
            };
            if last && tail.on() && let Some(e) = tail_expr {
                // Locals of the block are dropped once its value is computed; the function's
                // result leaves them all behind.
                if *tail == Tail::Return {
                    cur = self.drop_locals(block, cur, true);
                }
                cur = self.tail(s, e, cur, tail);
                ended_by_tail = true;
            } else {
                cur = self.stmt(s, cur);
            }
        }
        if !ended_by_tail || *tail != Tail::Return {
            cur = self.drop_locals(block, cur, false);
        }
        self.drops.pop();
        self.b.pop_scope();
        cur
    }

    /// Implicit `drop` of the block's locals whose types run code at the end of their scope
    /// (`impl Drop`, lock guards), in reverse order of declaration. Moved values are skipped.
    fn drop_locals(&mut self, block: Node, cur: Flow, returning: bool) -> Flow {
        let Some(locals) = self.drops.last_mut().map(std::mem::take) else { return cur };
        if cur.is_none() {
            return cur;
        }
        let mut cur = cur;
        let end = block.end_byte().saturating_sub(1).max(block.start_byte());
        let span = self.src.index.span(self.src.file, end, block.end_byte());
        for local in locals.into_iter().rev() {
            if self.b.moved_from(&local.node, &local.pin) {
                continue;
            }
            let when = if returning { "when the function returns" } else { "at the end of its block" };
            let (label, method) = if is_guard(&local.ty) {
                (format!("unlock: {} goes out of scope {when}", local.name), None)
            } else {
                let bare = crate::syntax::bare_type_name(&local.ty);
                (format!("drop {} {when} (runs {bare}'s drop)", local.name), self.drop_types.get(&bare).cloned())
            };
            let mut spec = NodeSpec::new(Concept::Call, label, format!("}} // drop({})", local.name), span.clone()).flavor("implicit-drop");
            if let Some(m) = method {
                spec = spec.call(CallTarget { symbol: Some(m), display: format!("{}::drop", crate::syntax::bare_type_name(&local.ty)), external: false });
            }
            let id = self.place(spec, &cur);
            self.b.read_with(&id, &local.name, Some(Access::Move));
            cur = Some(self.b.exec_out(&id, "out", ""));
        }
        cur
    }

    /// The value a block ends with: the function's result, or the value of a `let`.
    fn tail(&mut self, s: Node, e: Node, cur: Flow, tail: &Tail) -> Flow {
        let e = unparen(e);
        match e.kind() {
            "if_expression" if e.child_by_field_name("alternative").is_some() => self.if_expr(s, e, cur, tail),
            "match_expression" => self.match_expr(s, e, cur, tail),
            "block" => self.block(e, cur, tail),
            "unsafe_block" => self.unsafe_block(s, e, cur, tail),
            "return_expression" | "break_expression" | "continue_expression" | "loop_expression"
            | "while_expression" | "for_expression" | "assignment_expression" | "compound_assignment_expr" | "unit_expression" => {
                self.expr_stmt(s, e, cur)
            }
            "macro_invocation" if PANICS.contains(&macro_name(self.src, e).as_str()) || UNIT_MACROS.contains(&macro_name(self.src, e).as_str()) => {
                self.expr_stmt(s, e, cur)
            }
            _ => match tail {
                Tail::Set(name) => self.set_value(s, e, name, cur),
                _ => self.return_value(s, Some(e), cur),
            },
        }
    }

    /// `name` takes the value a branch of `let name = if … { … } else { … }` ends with.
    fn set_value(&mut self, s: Node, e: Node, name: &str, cur: Flow) -> Flow {
        let label = format!("{name} = {}", self.ex.natural(e));
        let lowered = Some(self.ex.lower(e));
        let spec = NodeSpec::new(Concept::Assign, self.label(label), self.src.norm(s), self.src.span(s)).flavor("block-value").expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, Some(e));
        self.propagate(&id, Some(e));
        self.b.write(&id, name);
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn stmt(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        match s.kind() {
            "let_declaration" => self.let_declaration(s, cur),
            "expression_statement" => match named_children(s).into_iter().next() {
                Some(e) => self.expr_stmt(s, e, cur),
                None => cur,
            },
            "function_item" => {
                let name = s.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let params: Vec<String> = parameters(src, s).into_iter().map(|(n, _)| n).collect();
                let body = s.child_by_field_name("body");
                let mut spec = NodeSpec::new(
                    Concept::Lambda,
                    self.label(format!("define function {name}({})", params.join(", "))),
                    src.header(s, body),
                    src.header_span(s, body),
                )
                .flavor("local-function");
                if let Some(id) = self.functions.get(&s.start_byte()) {
                    spec = spec.call(CallTarget { symbol: Some(id.clone()), display: format!("{name}({})", params.join(", ")), external: false });
                }
                let id = self.place(spec, &cur);
                self.b.define(&id, &name, None);
                Some(self.b.exec_out(&id, "out", ""))
            }
            k if is_item(k) => cur,
            k if is_expression(k) => self.expr_stmt(s, s, cur),
            _ => cur,
        }
    }

    fn let_declaration(&mut self, s: Node, cur: Flow) -> Flow {
        let src = self.src;
        let pattern = s.child_by_field_name("pattern");
        let value = s.child_by_field_name("value");
        let ty = s.child_by_field_name("type").map(|t| src.norm(t)).or_else(|| value.and_then(|v| self.infer(v)));
        let alternative = s.child_by_field_name("alternative");
        let single = pattern.and_then(|p| single_name(src, p));
        // `let x = if … { …; a } else { b }`, `let x = loop { … break v; }`, `let x = 'b: { … }`:
        // the control flow is drawn, and each way out sets `x`.
        if let (Some(n), Some(v)) = (&single, value)
            && alternative.is_none()
            && has_inner_statements(v)
        {
            return self.let_with_flow(s, n, v, ty, cur);
        }
        let value_text = value.map(|v| self.ex.natural(v));
        let mut flavors = Vec::new();
        if pattern.is_some_and(|p| p.kind() == "mut_pattern") {
            flavors.push("mut".to_owned());
        }
        if alternative.is_some() {
            flavors.push("let-else".to_owned());
        }
        if value.is_some_and(|v| unparen(v).kind() == "await_expression") {
            flavors.push("async".into());
        }
        // `let handle = thread::spawn(…)`: starting other work, keeping a handle to it.
        let spawn = value.map(unparen).filter(|v| v.kind() == "call_expression").and_then(|v| {
            let path = callee_path(src, v)?;
            let (_, how) = SPAWNS.iter().find(|(p, _)| *p == path)?;
            let arg = v.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next());
            Some(format!("run {} {how}", arg.map(|a| self.ex.natural(a)).unwrap_or_default()))
        });
        let shown = single.as_ref().map(|n| self.next_version(n));
        let (concept, label) = match (&shown, &value_text) {
            (Some(n), Some(_)) if spawn.is_some() => (Concept::Spawn, format!("{n} = {}", spawn.clone().unwrap_or_default())),
            (Some(n), Some(v)) => (Concept::Var, format!("{n} = {v}")),
            (Some(n), None) => (Concept::Var, format!("declare {n}")),
            (None, v) => (
                Concept::Destructure,
                match (v, alternative.is_some()) {
                    (Some(v), true) => format!("if {v} matches {}, unpack it", pattern.map(|p| src.norm(p)).unwrap_or_default()),
                    (Some(v), false) => format!("unpack {} from {v}", pattern.map(|p| src.norm(p)).unwrap_or_default()),
                    (None, _) => format!("declare {}", pattern.map(|p| src.norm(p)).unwrap_or_default()),
                },
            ),
        };
        let code = match alternative {
            Some(a) => src.header(s, Some(a)),
            None => src.norm(s),
        };
        let lowered = value.map(|v| self.ex.lower(v));
        let spec = NodeSpec::new(concept, self.label(label), code, src.span(s)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, value);
        self.propagate(&id, value);
        if let Some(a) = alternative {
            let other = self.b.exec_out(&id, "else", "Otherwise");
            let body = first_child_of_kind(a, "block").unwrap_or(a);
            self.block(body, Some(other), &Tail::No);
        }
        match (&single, pattern) {
            (Some(n), _) => self.define_local(&id, n, ty),
            (None, Some(p)) => {
                let mut names = Vec::new();
                pattern_names(src, p, &mut names);
                for n in names {
                    self.define_local(&id, &n, None);
                }
            }
            _ => {}
        }
        Some(self.b.exec_out(&id, "out", ""))
    }

    /// `name` as shown for its next definition: `value`, then `value₂`, `value₃` (shadowing).
    fn next_version(&self, name: &str) -> String {
        match self.defined.get(name) {
            Some(n) => format!("{name}{}", subscript(n + 1)),
            None => name.to_owned(),
        }
    }

    /// Defines a local, numbering shadowed versions and remembering locals that run code at
    /// the end of their scope.
    fn define_local(&mut self, id: &String, name: &str, ty: Option<String>) {
        let count = {
            let c = self.defined.entry(name.to_owned()).or_insert(0);
            *c += 1;
            *c
        };
        self.b.define(id, name, ty.clone());
        let pin = format!("var:{name}");
        if count > 1 {
            self.b.set_pin_label(id, &pin, &format!("{name}{}", subscript(count)));
        }
        if let Some(ty) = ty
            && (is_guard(&ty) || self.drop_types.contains_key(&crate::syntax::bare_type_name(&ty)))
            && let Some(scope) = self.drops.last_mut()
        {
            scope.push(DropLocal { name: name.to_owned(), ty, node: id.clone(), pin });
        }
    }

    /// `let name = <control flow>;`: a declaration, then the flow, whose exits set `name`.
    fn let_with_flow(&mut self, s: Node, name: &str, v: Node, ty: Option<String>, cur: Flow) -> Flow {
        let src = self.src;
        let v = unparen(v);
        let what = match v.kind() {
            "loop_expression" => "the loop",
            "if_expression" => "the if",
            "match_expression" => "the match",
            _ => "the block",
        };
        let shown = self.next_version(name);
        let head = src.header(s, Some(v));
        let spec = NodeSpec::new(Concept::Var, self.label(format!("{shown} = the value {what} below gives")), format!("{head} …"), src.header_span(s, Some(v)))
            .flavor("block-value");
        let id = self.place(spec, &cur);
        self.define_local(&id, name, ty);
        let cur = Some(self.b.exec_out(&id, "out", ""));
        let tail = Tail::Set(name.to_owned());
        match v.kind() {
            "if_expression" => self.if_expr(v, v, cur, &tail),
            "match_expression" => self.match_expr(v, v, cur, &tail),
            "unsafe_block" => self.unsafe_block(v, v, cur, &tail),
            "loop_expression" | "while_expression" | "for_expression" => {
                let before = self.target.replace(name.to_owned());
                let end = self.loop_expr(v, v, cur);
                self.target = before;
                end
            }
            _ => {
                let block = if v.kind() == "block" { v } else { first_child_of_kind(v, "block").unwrap_or(v) };
                // A labelled block: `break 'b value` leaves it with that value.
                let before = self.target.replace(name.to_owned());
                let end = self.block(block, cur, &tail);
                self.target = before;
                end
            }
        }
    }

    /// The type of a value when the code shows it: literals, casts, constructors, other locals
    /// and a few well-known methods. Semantic analysis fills in the rest.
    fn infer(&self, v: Node) -> Option<String> {
        let src = self.src;
        let v = unparen(v);
        if let Some(t) = literal_type(src, v) {
            return Some(t);
        }
        match v.kind() {
            "type_cast_expression" => v.child_by_field_name("type").map(|t| src.norm(t)),
            "identifier" => self.b.type_of(src.text(v)),
            "reference_expression" => {
                let inner = v.child_by_field_name("value")?;
                let mutable = children(v).iter().any(|c| c.kind() == "mutable_specifier");
                let t = self.infer(inner).unwrap_or_else(|| "…".into());
                Some(if mutable { format!("&mut {t}") } else { format!("&{t}") })
            }
            "struct_expression" => v.child_by_field_name("name").map(|n| src.norm(n)),
            "array_expression" => Some("[…]".into()),
            "macro_invocation" => match macro_name(src, v).as_str() {
                "format" => Some("String".into()),
                "vec" => Some("Vec<…>".into()),
                _ => None,
            },
            "binary_expression" => {
                let op = v.child_by_field_name("operator").map(|o| src.text(o))?;
                if matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||") {
                    return Some("bool".into());
                }
                let left = v.child_by_field_name("left").and_then(|l| self.infer(l));
                let right = v.child_by_field_name("right").and_then(|r| self.infer(r));
                match (left, right) {
                    (Some(l), _) if !l.starts_with('{') => Some(l),
                    (_, Some(r)) if !r.starts_with('{') => Some(r),
                    (Some(l), _) => Some(l),
                    (_, r) => r,
                }
            }
            "call_expression" => {
                if let Some((recv, name)) = crate::expr::method_call(v) {
                    return match src.text(name) {
                        "len" | "count" | "capacity" | "count_ones" => Some("usize".into()),
                        "is_empty" | "contains" | "starts_with" | "ends_with" | "is_some" | "is_none" | "is_ok" | "is_err"
                        | "any" | "all" | "contains_key" | "eq" | "ne" => Some("bool".into()),
                        "to_string" | "to_owned" if self.infer(recv).is_some_and(|t| t.contains("str")) => Some("String".into()),
                        "to_string" | "to_uppercase" | "to_lowercase" | "repeat" | "join" => Some("String".into()),
                        "clone" | "to_owned" => self.infer(recv).map(|t| t.trim_start_matches('&').to_owned()),
                        "unwrap" | "expect" if unparen(recv).kind() == "call_expression"
                            && crate::expr::method_call(unparen(recv)).is_some_and(|(_, n)| matches!(src.text(n), "lock" | "try_lock")) =>
                        {
                            Some("MutexGuard<…>".into())
                        }
                        "write" | "read" if self.infer(recv).is_some_and(|t| t.contains("RwLock")) => Some("RwLockGuard<…>".into()),
                        "borrow_mut" => Some("RefMut<…>".into()),
                        _ => None,
                    };
                }
                let path = callee_path(src, v)?;
                let segs: Vec<&str> = path.split("::").collect();
                if segs.len() >= 2 {
                    let ty = segs[segs.len() - 2];
                    let ctor = segs[segs.len() - 1];
                    // `Rc::clone(&shared)`: the same type as what it clones.
                    if ctor == "clone"
                        && let Some(arg) = v.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next())
                    {
                        return self.infer(arg).map(|t| t.trim_start_matches('&').to_owned());
                    }
                    if ty.chars().next().is_some_and(char::is_uppercase)
                        && matches!(ctor, "new" | "with_capacity" | "default" | "from" | "new_cyclic")
                    {
                        let base = ty.split('<').next().unwrap_or(ty);
                        return Some(match base {
                            "String" => "String".into(),
                            "Box" | "Rc" | "Arc" | "RefCell" | "Cell" | "Mutex" | "RwLock" | "Vec" | "HashMap" | "HashSet" | "BTreeMap"
                            | "VecDeque" => format!("{base}<…>"),
                            _ => base.to_owned(),
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn expr_stmt(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let e = unparen(e);
        match e.kind() {
            "if_expression" => self.if_expr(s, e, cur, &Tail::No),
            "match_expression" => self.match_expr(s, e, cur, &Tail::No),
            "loop_expression" | "while_expression" | "for_expression" => self.loop_expr(s, e, cur),
            "block" => self.block(e, cur, &Tail::No),
            "unsafe_block" => self.unsafe_block(s, e, cur, &Tail::No),
            "return_expression" => {
                let v = named_children(e).into_iter().next();
                self.return_value(s, v, cur)
            }
            "break_expression" | "continue_expression" => {
                let label_name = first_child_of_kind(e, "label").map(|l| src.norm(l));
                let value = named_children(e).into_iter().find(|c| c.kind() != "label");
                let is_break = e.kind() == "break_expression";
                let mut flavors = Vec::new();
                if let Some(l) = &label_name {
                    flavors.push(format!("label:{l}"));
                }
                let into = self.target.as_ref().map(|t| format!(" ({t} takes it)")).unwrap_or_default();
                let label = match (is_break, &label_name, value) {
                    (true, Some(l), Some(v)) => format!("leave {l} with {}{into}", self.ex.natural(v)),
                    (true, None, Some(v)) => format!("leave the loop with {}{into}", self.ex.natural(v)),
                    (true, Some(l), None) => format!("leave {l}"),
                    (true, None, None) => "leave the loop".to_owned(),
                    (false, Some(l), _) => format!("continue with the next iteration of {l}"),
                    (false, None, _) => "skip to the next iteration".to_owned(),
                };
                let concept = if is_break { Concept::Break } else { Concept::Continue };
                self.simple(s, concept, label, value, flavors, cur)
            }
            "assignment_expression" | "compound_assignment_expr" => self.assignment(s, e, cur),
            "macro_invocation" => self.macro_stmt(s, e, cur),
            "call_expression" => self.call_stmt(s, e, cur),
            "await_expression" => {
                let inner = named_children(e).into_iter().next().unwrap_or(e);
                if inner.kind() == "call_expression"
                    && let Some(path) = callee_path(src, inner)
                    && SLEEPS.contains(&path.as_str())
                {
                    let arg = inner.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next());
                    let label = format!("wait {}", arg.map(|a| duration_text(src, a).unwrap_or_else(|| self.ex.natural(a))).unwrap_or_default());
                    return self.simple(s, Concept::Wait, label, Some(e), vec!["async".into()], cur);
                }
                let label = format!("wait for {}", self.ex.natural(inner));
                self.simple(s, Concept::Await, label, Some(e), Vec::new(), cur)
            }
            "try_expression" => {
                let inner = named_children(e).into_iter().next().unwrap_or(e);
                let flow = if unparen(inner).kind() == "call_expression" {
                    self.call_stmt(s, unparen(inner), cur)
                } else {
                    let label = self.ex.natural(e);
                    self.simple(s, Concept::Call, label, Some(inner), Vec::new(), cur)
                };
                // The `?` is outside the inner call: mark the node placed for it.
                if let Some((id, _)) = &flow {
                    let id = id.clone();
                    if self.b.concept_of(&id).is_some() {
                        self.b.exec_out(&id, "error", "If it fails (Err or None): return it right away");
                        if let Some(i) = self.b.index_of(&id) {
                            self.b.add_flavor_at(i, "try");
                        }
                    }
                }
                flow
            }
            "yield_expression" => {
                let value = named_children(e).into_iter().next();
                let label = format!("produce {}", value.map(|v| self.ex.natural(v)).unwrap_or_default());
                self.simple(s, Concept::Yield, label, value, Vec::new(), cur)
            }
            "closure_expression" => {
                let label = format!("create {}", self.ex.natural(e));
                self.simple(s, Concept::Lambda, label, Some(e), Vec::new(), cur)
            }
            "async_block" => {
                let label = "create an async block (it runs when awaited)".to_owned();
                self.simple(s, Concept::Lambda, label, Some(e), vec!["async".into()], cur)
            }
            "unit_expression" => cur,
            _ => {
                let label = self.ex.natural(e);
                self.simple(s, Concept::Call, label, Some(e), Vec::new(), cur)
            }
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
                if self.ex.pipeline(bare).is_some() {
                    flavors.push("iterator".to_owned());
                }
                format!("return {}", self.ex.natural(e))
            }
            None => "return".to_owned(),
        };
        self.simple(s, Concept::Return, label, e, flavors, cur)
    }

    fn unsafe_block(&mut self, s: Node, e: Node, cur: Flow, tail: &Tail) -> Flow {
        let src = self.src;
        let body = first_child_of_kind(e, "block");
        let spec = NodeSpec::new(Concept::Scope, "run this block without the compiler's safety checks", "unsafe".to_owned(), src.header_span(s, body)).flavor("unsafe");
        let id = self.place(spec, &cur);
        let b = self.b.exec_out(&id, "body", "Body");
        let end = match body {
            Some(body) => self.block(body, Some(b), tail),
            None => Some(b),
        };
        if tail.on() && end.is_none() {
            return None;
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    /// The names bound by `if let` / `while let` conditions (also inside `&&` chains).
    fn condition_names(&self, cond: Node, out: &mut Vec<String>) {
        match cond.kind() {
            "let_condition" => {
                if let Some(p) = cond.child_by_field_name("pattern") {
                    pattern_names(self.src, p, out);
                }
            }
            "let_chain" => {
                for c in named_children(cond) {
                    self.condition_names(c, out);
                }
            }
            _ => {}
        }
    }

    fn condition_label(&self, cond: Node) -> String {
        let src = self.src;
        match cond.kind() {
            "let_condition" => {
                let pattern = cond.child_by_field_name("pattern").map(|p| src.norm(p)).unwrap_or_default();
                let value = cond.child_by_field_name("value").map(|v| self.ex.natural(v)).unwrap_or_default();
                format!("{value} matches {pattern}")
            }
            "let_chain" => named_children(cond).into_iter().map(|c| self.condition_label(c)).collect::<Vec<_>>().join(" and "),
            _ => self.ex.natural(cond),
        }
    }

    fn if_expr(&mut self, s: Node, e: Node, cur: Flow, tail: &Tail) -> Flow {
        let src = self.src;
        let cond = e.child_by_field_name("condition").map(unparen);
        let cons = e.child_by_field_name("consequence");
        let alt = e.child_by_field_name("alternative").and_then(|a| named_children(a).into_iter().next());
        let label = format!("if {}", cond.map(|c| self.condition_label(c)).unwrap_or_default());
        let mut flavors = Vec::new();
        if cond.is_some_and(|c| matches!(c.kind(), "let_condition" | "let_chain")) {
            flavors.push("if-let".to_owned());
        }
        let lowered = cond.map(|c| self.ex.lower(c));
        let code = if s.id() == e.id() || s.kind() == "expression_statement" { src.header(e, cons) } else { src.header(s, cons) };
        let spec = NodeSpec::new(Concept::Branch, self.label(label), code, src.header_span(e, cons)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, cond);
        self.propagate(&id, cond);
        let t = self.b.exec_out(&id, "true", "True");
        let f = self.b.exec_out(&id, "false", "False");
        self.b.push_scope();
        if let Some(c) = cond {
            let mut names = Vec::new();
            self.condition_names(c, &mut names);
            for n in names {
                self.b.define(&id, &n, None);
            }
        }
        let t_end = match cons {
            Some(c) => self.block(c, Some(t), tail),
            None => Some(t),
        };
        self.b.pop_scope();
        let f_end = match alt {
            Some(a) if a.kind() == "if_expression" => self.if_expr(a, a, Some(f), tail),
            Some(a) => self.block(a, Some(f), tail),
            None => Some(f),
        };
        if alt.is_some() && t_end.is_none() && f_end.is_none() {
            return None;
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn match_expr(&mut self, s: Node, e: Node, cur: Flow, tail: &Tail) -> Flow {
        let src = self.src;
        let value = e.child_by_field_name("value").map(unparen);
        let body = e.child_by_field_name("body");
        let label = format!("match {}", value.map(|v| self.ex.natural(v)).unwrap_or_default());
        let lowered = value.map(|v| self.ex.lower(v));
        let code = if s.kind() == "expression_statement" || s.id() == e.id() { src.header(e, body) } else { src.header(s, body) };
        let spec = NodeSpec::new(Concept::Match, self.label(label), code, src.header_span(e, body)).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, value);
        self.propagate(&id, value);
        let arms: Vec<Node> = body.map(|b| named_children(b).into_iter().filter(|a| a.kind() == "match_arm").collect()).unwrap_or_default();
        let mut any_continues = arms.is_empty();
        for (i, arm) in arms.iter().enumerate() {
            let pattern = arm.child_by_field_name("pattern");
            let pin_label = truncate(&pattern.map(|p| src.norm(p)).unwrap_or_default(), 40);
            let pin = self.b.exec_out(&id, &format!("case:{i}"), &pin_label);
            self.b.push_scope();
            if let Some(p) = pattern {
                let mut names = Vec::new();
                // `match_pattern`: the pattern, then an optional `if` guard.
                for c in named_children(p) {
                    if p.child_by_field_name("condition").is_some_and(|g| g.id() == c.id()) {
                        continue;
                    }
                    pattern_names(src, c, &mut names);
                }
                for n in names {
                    self.b.define(&id, &n, None);
                }
                if let Some(g) = p.child_by_field_name("condition") {
                    self.read_all(&id, Some(g));
                }
            }
            let end = match arm.child_by_field_name("value") {
                Some(v) if v.kind() == "block" => self.block(v, Some(pin), tail),
                Some(v) if tail.on() => self.tail(*arm, v, Some(pin), tail),
                Some(v) => self.expr_stmt(*arm, v, Some(pin)),
                None => Some(pin),
            };
            self.b.pop_scope();
            any_continues |= end.is_some();
        }
        if !any_continues {
            return None;
        }
        Some(self.b.exec_out(&id, "out", "Then"))
    }

    fn loop_expr(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let body = e.child_by_field_name("body");
        let mut flavors = Vec::new();
        if let Some(l) = first_child_of_kind(e, "label") {
            flavors.push(format!("label:{}", src.norm(l)));
        }
        let code = src.header(e, body);
        let span = src.header_span(e, body);
        let _ = s;
        let (id, defines) = match e.kind() {
            "loop_expression" => {
                let spec = NodeSpec::new(Concept::LoopInfinite, "repeat forever", code, span).flavors(flavors);
                (self.place(spec, &cur), Vec::new())
            }
            "while_expression" => {
                let cond = e.child_by_field_name("condition").map(unparen);
                let text = cond.map(|c| self.condition_label(c)).unwrap_or_default();
                let mut names = Vec::new();
                if let Some(c) = cond {
                    self.condition_names(c, &mut names);
                    if matches!(c.kind(), "let_condition" | "let_chain") {
                        flavors.push("while-let".into());
                    }
                }
                let (concept, label) = if text == "true" { (Concept::LoopInfinite, "repeat forever".to_owned()) } else { (Concept::LoopWhile, format!("while {text}")) };
                let lowered = cond.map(|c| self.ex.lower(c));
                let spec = NodeSpec::new(concept, self.label(label), code, span).flavors(flavors).expr(lowered);
                let id = self.place(spec, &cur);
                self.read_all(&id, cond);
                self.propagate(&id, cond);
                (id, names)
            }
            _ => {
                let pattern = e.child_by_field_name("pattern");
                let value = e.child_by_field_name("value");
                let label = format!(
                    "for each {} in {}",
                    pattern.map(|p| src.norm(p)).unwrap_or_default(),
                    value.map(|v| self.ex.natural(v)).unwrap_or_default()
                );
                let mut names = Vec::new();
                if let Some(p) = pattern {
                    pattern_names(src, p, &mut names);
                }
                if value.is_some_and(|v| unparen(v).kind() == "range_expression") {
                    flavors.push("range".into());
                }
                let lowered = value.map(|v| self.ex.lower(v));
                let spec = NodeSpec::new(Concept::LoopForEach, self.label(label), code, span).flavors(flavors).expr(lowered);
                let id = self.place(spec, &cur);
                self.read_all(&id, value);
                self.propagate(&id, value);
                (id, names)
            }
        };
        self.b.push_scope();
        for n in defines {
            self.b.define(&id, &n, None);
        }
        let b = self.b.exec_out(&id, "body", "Loop body");
        if let Some(body) = body {
            self.block(body, Some(b), &Tail::No);
        }
        self.b.pop_scope();
        Some(self.b.exec_out(&id, "out", "Completed"))
    }

    fn assignment(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let left = e.child_by_field_name("left");
        let right = e.child_by_field_name("right");
        let op = e.child_by_field_name("operator").map(|o| src.text(o).to_owned()).unwrap_or_else(|| "=".into());
        let left_text = left.map(|l| src.norm(l)).unwrap_or_default();
        let right_text = right.map(|r| self.ex.natural(r)).unwrap_or_default();
        let mut flavors = Vec::new();
        let concept = match left.map(|l| l.kind()) {
            Some("identifier") => Concept::Assign,
            Some("index_expression") => {
                flavors.push("index".to_owned());
                Concept::Assign
            }
            Some("unary_expression") => {
                flavors.push("deref".to_owned());
                Concept::Assign
            }
            Some("tuple_expression") => Concept::Destructure,
            _ => Concept::SetMember,
        };
        let label = match op.as_str() {
            "=" => format!("set {left_text} to {right_text}"),
            "+=" => format!("add {right_text} to {left_text}"),
            "-=" => format!("subtract {right_text} from {left_text}"),
            "*=" => format!("multiply {left_text} by {right_text}"),
            "/=" => format!("divide {left_text} by {right_text}"),
            o => format!("{left_text} {o} {right_text}"),
        };
        let label = if op == "+=" && right_text == "1" { format!("increase {left_text} by 1") } else if op == "-=" && right_text == "1" { format!("decrease {left_text} by 1") } else { label };
        let lowered = Some(self.ex.lower(e));
        let spec = NodeSpec::new(concept, self.label(label), src.norm(s), src.span(s)).flavors(flavors).expr(lowered);
        let id = self.place(spec, &cur);
        self.read_all(&id, right);
        // `total += x` reads `total` too; `items[i] = x` reads `items` and `i`.
        if op != "=" || left.is_some_and(|l| l.kind() != "identifier") {
            self.read_all(&id, left);
        }
        self.propagate(&id, Some(e));
        if let Some(l) = left {
            let target = match l.kind() {
                "identifier" => Some(src.norm(l)),
                "index_expression" | "field_expression" => named_children(l).into_iter().next().filter(|b| b.kind() == "identifier").map(|b| src.norm(b)),
                _ => None,
            };
            if let Some(t) = target {
                self.b.write(&id, &t);
            }
        }
        Some(self.b.exec_out(&id, "out", ""))
    }

    fn macro_stmt(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        let name = macro_name(src, e);
        let args: Vec<String> = macro_args(src, e).into_iter().map(|(a, b)| nodex_lang_api::kit::normalize_ws(&src.text[a..b])).collect();
        let joined = args.join(", ");
        if PANICS.contains(&name.as_str()) {
            let label = match name.as_str() {
                "todo" => "stop: not written yet".to_owned(),
                "unimplemented" => "stop: not implemented".to_owned(),
                "unreachable" => "stop: this should never happen".to_owned(),
                _ if joined.is_empty() => "stop the program (panic)".to_owned(),
                _ => format!("stop the program (panic): {joined}"),
            };
            return self.simple(s, Concept::Throw, label, Some(e), vec!["panic".into()], cur);
        }
        let (concept, label, flavors) = match name.as_str() {
            "println" | "print" => (Concept::Call, format!("print {joined}"), vec!["macro".to_owned()]),
            "eprintln" | "eprint" => (Concept::Call, format!("print to the error output {joined}"), vec!["macro".to_owned()]),
            "assert" | "debug_assert" => (Concept::Call, format!("check that {joined}"), vec!["macro".to_owned(), "assert".to_owned()]),
            "assert_eq" | "debug_assert_eq" => (
                Concept::Call,
                format!("check that {} equals {}", args.first().cloned().unwrap_or_default(), args.get(1).cloned().unwrap_or_default()),
                vec!["macro".to_owned(), "assert".to_owned()],
            ),
            "assert_ne" | "debug_assert_ne" => (
                Concept::Call,
                format!("check that {} differs from {}", args.first().cloned().unwrap_or_default(), args.get(1).cloned().unwrap_or_default()),
                vec!["macro".to_owned(), "assert".to_owned()],
            ),
            "write" | "writeln" => (Concept::Call, format!("write to {}", args.first().cloned().unwrap_or_default()), vec!["macro".to_owned()]),
            n if KNOWN_MACROS.contains(&n) => (Concept::Call, src.norm(e), vec!["macro".to_owned()]),
            "bail" => (Concept::Throw, format!("fail with {joined}"), vec!["macro".to_owned()]),
            _ => (Concept::Macro, src.norm(e), Vec::new()),
        };
        self.simple(s, concept, label, Some(e), flavors, cur)
    }

    fn call_stmt(&mut self, s: Node, e: Node, cur: Flow) -> Flow {
        let src = self.src;
        if let Some(path) = callee_path(src, e) {
            let arg = e.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next());
            if let Some((_, how)) = SPAWNS.iter().find(|(p, _)| *p == path) {
                let what = arg.map(|a| self.ex.natural(a)).unwrap_or_default();
                let label = format!("run {what} {how}");
                return self.simple(s, Concept::Spawn, label, Some(e), Vec::new(), cur);
            }
            if SLEEPS.contains(&path.as_str()) {
                let label = format!("wait {}", arg.map(|a| duration_text(src, a).unwrap_or_else(|| self.ex.natural(a))).unwrap_or_default());
                return self.simple(s, Concept::Wait, label, Some(e), vec!["blocking".into()], cur);
            }
            if matches!(path.as_str(), "std::process::exit" | "process::exit" | "exit") {
                let label = format!("end the program with exit code {}", arg.map(|a| self.ex.natural(a)).unwrap_or_default());
                return self.simple(s, Concept::Return, label, Some(e), vec!["exits".into()], cur);
            }
            if matches!(path.as_str(), "drop" | "std::mem::drop" | "mem::drop") {
                let label = format!("drop {} (free it now)", arg.map(|a| self.ex.natural(a)).unwrap_or_default());
                return self.simple(s, Concept::Call, label, Some(e), vec!["drop".into()], cur);
            }
        }
        // `tx.send(x)` (also `tx.send(x).unwrap()`): sending a value through a channel.
        let mut send = e;
        while let Some((recv, name)) = method_call(send)
            && matches!(src.text(name), "unwrap" | "expect" | "ok")
            && unparen(recv).kind() == "call_expression"
        {
            send = unparen(recv);
        }
        if let Some((_, name)) = method_call(send) {
            let name = src.norm(name);
            if name == "send" && self.ex.pipeline(send).is_none() {
                let what = send.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next()).map(|a| self.ex.natural(a)).unwrap_or_default();
                let target = method_call(send).map(|(r, _)| self.ex.natural(r)).unwrap_or_default();
                let label = format!("send {what} through {target}");
                return self.simple(s, Concept::Emit, label, Some(e), vec!["channel".into()], cur);
            }
        }
        let mut flavors = Vec::new();
        if self.ex.pipeline(e).is_some() {
            flavors.push("iterator".to_owned());
        }
        let label = self.ex.natural(e);
        let _ = children;
        self.simple(s, Concept::Call, label, Some(e), flavors, cur)
    }
}
