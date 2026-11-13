//! Recognition of library calls as shared operations (`catalog/operations.toml`).
//!
//! Operators are tagged by each frontend (`nodex_lang_api::ops`). Library calls are recognized
//! here, once for every language: `Math.Max(a, b)`, `Math.max(a, b)` and `a.max(b)` all become
//! the operation `math.max`. Patterns come from the catalog's `calls_cs` / `calls_ts` / `calls_rs`.
//!
//! Recognition is idempotent: it runs on every flow the server hands out, after semantic
//! enrichment, so calls resolved to a project method (`Expr::symbol`) are never mistaken for a
//! library operation that happens to share its name.

use std::collections::HashMap;
use std::sync::OnceLock;

use nodex_ir::{Concept, Expr, FlowGraph, Language};

use crate::catalog::{Catalog, OperationInfo};

/// One call pattern: `Math.Max()`, `*.ToUpper()`, `*.Length`, `parseInt()`, `rand::random()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    /// Receiver path (`Math`, `Console.Error`); `None` for `*` (any receiver); `Some("")` for a
    /// free function.
    pub receiver: Option<String>,
    pub name: String,
    /// `true` for a call (`()`), `false` for a property read.
    pub call: bool,
    /// `~random|rng.Next()`: any receiver whose name contains one of these (lower-case) words,
    /// for member names too common to trust on any receiver.
    pub hint: Vec<String>,
}

impl Pattern {
    pub fn parse(text: &str) -> Option<Self> {
        let (body, call) = match text.strip_suffix("()") {
            Some(b) => (b, true),
            None => (text, false),
        };
        if body.is_empty() || body.contains(char::is_whitespace) {
            return None;
        }
        let split = body.rfind("::").map(|i| (i, 2)).into_iter().chain(body.rfind('.').map(|i| (i, 1))).max_by_key(|(i, _)| *i);
        let (receiver, name) = match split {
            Some((i, len)) => (&body[..i], &body[i + len..]),
            None => ("", body),
        };
        if name.is_empty() || (receiver.is_empty() && split.is_some()) {
            return None;
        }
        if !call && receiver.is_empty() {
            // A bare name without `()` would match every variable.
            return None;
        }
        if let Some(words) = receiver.strip_prefix('~') {
            let hint: Vec<String> = words.split('|').map(str::trim).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
            if hint.is_empty() {
                return None;
            }
            return Some(Self { receiver: None, name: name.to_owned(), call, hint });
        }
        let receiver = match receiver {
            "*" => None,
            r => Some(r.replace("::", ".")),
        };
        Some(Self { receiver, name: name.to_owned(), call, hint: Vec::new() })
    }
}

/// Patterns of one language, indexed by member name.
struct Index {
    by_name: HashMap<String, Vec<(Pattern, String)>>,
}

fn language_key(language: Language) -> &'static str {
    match language {
        Language::Csharp => "csharp",
        Language::Typescript => "typescript",
        Language::Rust => "rust",
    }
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(Catalog::embedded)
}

fn index(language: Language) -> &'static Index {
    static INDEXES: OnceLock<HashMap<&'static str, Index>> = OnceLock::new();
    let all = INDEXES.get_or_init(|| {
        let mut out = HashMap::new();
        for lang in [Language::Csharp, Language::Typescript, Language::Rust] {
            let mut by_name: HashMap<String, Vec<(Pattern, String)>> = HashMap::new();
            for (key, op) in &catalog().operations {
                for text in op.calls(language_key(lang)) {
                    if let Some(p) = Pattern::parse(text) {
                        by_name.entry(p.name.clone()).or_default().push((p, key.clone()));
                    }
                }
            }
            // Exact receivers win over `*`.
            for list in by_name.values_mut() {
                list.sort_by_key(|(p, _)| p.receiver.is_none());
            }
            out.insert(language_key(lang), Index { by_name });
        }
        out
    });
    &all[language_key(language)]
}

/// The catalog entry of `key`.
pub fn info(key: &str) -> Option<&'static OperationInfo> {
    catalog().operations.get(key)
}

/// Tags every recognized library call in `graph` with its operation.
pub fn recognize_flow(graph: &mut FlowGraph) {
    let language = graph.language;
    for node in &mut graph.nodes {
        if let Some(expr) = &mut node.expr {
            recognize(expr, language);
        }
    }
}

/// Tags `expr` and everything below it.
pub fn recognize(expr: &mut Expr, language: Language) {
    for child in &mut expr.children {
        recognize(child, language);
    }
    if expr.op.is_some() {
        return;
    }
    let Some(key) = match_expr(expr, language) else { return };
    let Some(op) = info(&key) else { return };
    expr.op = Some(key);
    if let Some((_, ty)) = op.output.split_once(':')
        && ty != "any"
    {
        expr.ty = Some(ty.to_owned());
    }
    // Pipeline stages keep their place in the pipeline; impure calls stay on the exec chain.
    let stage = expr.flavors.iter().any(|f| f == "stage");
    if op.pure && !stage {
        expr.concept = Concept::Operation;
    }
}

/// The written path of a receiver: `Math`, `Console.Error`, `fs.promises`.
fn path(e: &Expr) -> Option<String> {
    match e.concept {
        Concept::Var => Some(strip_generics(&e.label).to_owned()),
        Concept::GetMember => {
            let name = e.label.strip_prefix('.')?;
            match e.children.first() {
                Some(inner) => Some(format!("{}.{}", path(inner)?, strip_generics(name))),
                None => Some(strip_generics(name).to_owned()),
            }
        }
        _ => None,
    }
}

fn strip_generics(name: &str) -> &str {
    name.split('<').next().unwrap_or(name).trim()
}

/// The receiver of a hinted pattern looks like one of its words: `_random`, `this.rng`,
/// `new Random()`, `Random.Shared`; with semantics, the call must be the standard library's.
fn hint_agrees(e: &Expr, pattern: &Pattern, language: Language) -> bool {
    if let Some(t) = e.target.as_deref() {
        return library_target(t, language);
    }
    let Some(r) = e.children.first().filter(|c| c.flavors.iter().any(|f| f == "target")) else { return false };
    let written = match r.concept {
        Concept::Construct => r.label.trim_start_matches("new ").to_owned(),
        _ => path(r).unwrap_or_default(),
    };
    // Words of the receiver's name (`_sharedRandom` → shared, random; `RNG` → rng); a word must
    // start with a hint (`random`, `randomizer`), so `brand` or `operand` do not count as `rand`.
    let last = written.rsplit('.').next().unwrap_or(&written);
    let words = name_words(last);
    let all = if r.concept == Concept::Construct || written.contains('.') { name_words(&written) } else { Vec::new() };
    pattern.hint.iter().any(|h| words.iter().chain(&all).any(|w| w.starts_with(h.as_str())))
}

/// `_sharedRandom` → `["shared", "random"]`; `RNG` → `["rng"]`; `Random.Shared` → both.
fn name_words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur).to_lowercase());
            }
            continue;
        }
        let boundary = c.is_uppercase() && i > 0 && (chars[i - 1].is_lowercase() || chars.get(i + 1).is_some_and(|n| n.is_lowercase()) && chars[i - 1].is_uppercase());
        if boundary && !cur.is_empty() {
            out.push(std::mem::take(&mut cur).to_lowercase());
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur.to_lowercase());
    }
    out
}

fn match_expr(e: &Expr, language: Language) -> Option<String> {
    let (call, name, receiver) = match e.concept {
        Concept::Call => {
            let receiver = e.children.first().filter(|c| c.flavors.iter().any(|f| f == "target"));
            let label = strip_generics(&e.label);
            match receiver {
                Some(r) => (true, label.to_owned(), Some(path(r))),
                None => {
                    // Free call: `parseInt`, `Math.max` written as a whole (when the frontend kept
                    // the dotted callee), `println!`.
                    match label.rsplit_once('.') {
                        Some((recv, name)) => (true, name.to_owned(), Some(Some(recv.to_owned()))),
                        None => (true, label.replace("::", "."), None),
                    }
                }
            }
        }
        Concept::GetMember => {
            let name = e.label.strip_prefix('.')?.to_owned();
            let receiver = e.children.first().map(path);
            (false, name, receiver)
        }
        _ => return None,
    };
    // A free name like `env.var` written with `::` was flattened above; split it again.
    let (name, receiver) = match (receiver, name.rsplit_once('.')) {
        (None, Some((recv, n))) => (n.to_owned(), Some(Some(recv.to_owned()))),
        (r, _) => (name, r),
    };
    let candidates = index(language).by_name.get(&name)?;
    // Calls resolved to a project method are the project's, whatever their name.
    let project_symbol = e.symbol.is_some();
    for (pattern, key) in candidates {
        if pattern.call != call || (language == Language::Rust && rust_target_conflicts(key, e.target.as_deref())) {
            continue;
        }
        let ok = match (&pattern.receiver, &receiver) {
            // Free function: no receiver at all.
            (Some(r), None) => r.is_empty(),
            (Some(r), Some(Some(path))) => !r.is_empty() && (path == r || path.ends_with(&format!(".{r}"))) && !project_symbol,
            (Some(_), Some(None)) => false,
            // `*.Name`: any receiver, but only when the call is not a project method and, with
            // semantics, the method belongs to the standard library.
            (None, Some(_)) if !pattern.hint.is_empty() => !project_symbol && hint_agrees(e, pattern, language),
            (None, Some(_)) => !project_symbol && e.target.as_deref().is_none_or(|t| library_target(t, language)),
            (None, None) => false,
        };
        if ok && target_agrees(e, pattern) {
            return Some(key.clone());
        }
    }
    None
}

/// Calls that share a name with a catalog operation but have a different Rust library contract.
fn rust_target_conflicts(operation: &str, target: Option<&str>) -> bool {
    matches!((operation, target),
        ("list.flatten", Some("core::option::Option::flatten()"))
        | ("text.replace", Some("core::option::Option::replace()" | "core::mem::replace()"))
        | ("text.join", Some("std::path::Path::join()"))
    )
}

/// Standard-library containers whose methods `*.name` patterns may match once a semantic
/// provider resolved the call (`Array.push(...T[])`, `System.String.ToUpper()`).
const TS_LIBRARY: &[&str] = &[
    "String", "Array", "ReadonlyArray", "Map", "ReadonlyMap", "Set", "ReadonlySet", "WeakMap", "WeakSet", "Number",
    "Object", "ObjectConstructor", "ArrayConstructor", "NumberConstructor", "StringConstructor", "Date", "Promise",
    "PromiseConstructor", "Math", "JSON", "Console", "Storage", "URLSearchParams", "Headers", "FormData", "Performance",
    "Iterator", "IterableIterator", "Generator", "RegExp", "Boolean", "BigInt",
];

fn library_target(target: &str, language: Language) -> bool {
    let head = target.split('(').next().unwrap_or(target);
    let container = head.rsplit_once('.').map(|(c, _)| c).unwrap_or("");
    match language {
        Language::Csharp => ["System.", "UnityEngine.", "Newtonsoft.", "Unity."].iter().any(|p| head.starts_with(p)),
        Language::Typescript => {
            let bare = container.split('<').next().unwrap_or(container);
            TS_LIBRARY.contains(&bare) || bare.starts_with("NodeJS") || bare.is_empty()
        }
        Language::Rust => ["std::", "core::", "alloc::"].iter().any(|p| head.starts_with(p)),
    }
}

/// With semantics, the resolved target must agree with an exact pattern (`System.Math.Max(int,
/// int)` for `Math.Max()`).
fn target_agrees(e: &Expr, pattern: &Pattern) -> bool {
    let (Some(target), Some(receiver)) = (&e.target, &pattern.receiver) else { return true };
    if receiver.is_empty() {
        return true;
    }
    let head = target.split('(').next().unwrap_or(target);
    let want = format!("{receiver}.{}", pattern.name);
    head == want || head.ends_with(&format!(".{want}")) || !head.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_ir::Span;

    fn e(concept: Concept, label: &str, children: Vec<Expr>) -> Expr {
        Expr {
            syntax_sites: None,
            type_info: None,
            value_info: None,
            concept,
            label: label.into(),
            focus: None,
            span: Span::default(),
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

    fn target(mut x: Expr) -> Expr {
        x.flavors.push("target".into());
        x
    }

    fn call(recv: &str, name: &str, args: Vec<Expr>) -> Expr {
        let mut kids = vec![target(e(Concept::Var, recv, Vec::new()))];
        kids.extend(args);
        e(Concept::Call, name, kids)
    }

    #[test]
    fn patterns_parse() {
        let p = Pattern::parse("Math.Max()").unwrap();
        assert_eq!((p.receiver.as_deref(), p.name.as_str(), p.call), (Some("Math"), "Max", true));
        let p = Pattern::parse("*.Length").unwrap();
        assert_eq!((p.receiver, p.call), (None, false));
        let p = Pattern::parse("parseInt()").unwrap();
        assert_eq!(p.receiver.as_deref(), Some(""));
        let p = Pattern::parse("rand::random()").unwrap();
        assert_eq!(p.receiver.as_deref(), Some("rand"));
        assert!(Pattern::parse("length").is_none());
    }

    #[test]
    fn same_operation_in_both_languages() {
        let mut cs = call("Math", "Max", vec![e(Concept::Var, "a", vec![]), e(Concept::Var, "b", vec![])]);
        let mut ts = call("Math", "max", vec![e(Concept::Var, "a", vec![]), e(Concept::Var, "b", vec![])]);
        recognize(&mut cs, Language::Csharp);
        recognize(&mut ts, Language::Typescript);
        assert_eq!(cs.op.as_deref(), Some("math.max"));
        assert_eq!(ts.op, cs.op);
        assert_eq!(cs.concept, Concept::Operation);

        let mut cs = call("name", "ToUpper", vec![]);
        let mut ts = call("name", "toUpperCase", vec![]);
        recognize(&mut cs, Language::Csharp);
        recognize(&mut ts, Language::Typescript);
        assert_eq!(cs.op.as_deref(), Some("text.to_upper"));
        assert_eq!(ts.op, cs.op);

        let mut cs = e(Concept::GetMember, ".Count", vec![e(Concept::Var, "items", vec![])]);
        let mut ts = e(Concept::GetMember, ".length", vec![e(Concept::Var, "items", vec![])]);
        recognize(&mut cs, Language::Csharp);
        recognize(&mut ts, Language::Typescript);
        assert_eq!(cs.op.as_deref(), Some("text.length"));
        assert_eq!(ts.op, cs.op);
        assert_eq!(ts.ty.as_deref(), Some("number"));
    }

    #[test]
    fn impure_calls_stay_calls_and_project_methods_are_left_alone() {
        let console = e(Concept::GetMember, ".Error", vec![e(Concept::Var, "Console", vec![])]);
        let mut x = e(Concept::Call, "WriteLine", vec![target(console)]);
        recognize(&mut x, Language::Csharp);
        assert_eq!(x.op.as_deref(), Some("io.print_error"));
        assert_eq!(x.concept, Concept::Call);

        let mut mine = call("repo", "Add", vec![]);
        mine.symbol = Some("M:Repo.Add".into());
        recognize(&mut mine, Language::Csharp);
        assert_eq!(mine.op, None);

        let mut free = e(Concept::Call, "parseInt", vec![e(Concept::Var, "s", vec![])]);
        recognize(&mut free, Language::Typescript);
        assert_eq!(free.op.as_deref(), Some("convert.to_integer"));
        // `x.parseInt(s)` is not the free function.
        let mut member = call("x", "parseInt", vec![]);
        recognize(&mut member, Language::Typescript);
        assert_eq!(member.op, None);
    }

    #[test]
    fn next_is_random_only_on_a_random_generator() {
        let p = Pattern::parse("~random|rng.Next()").unwrap();
        assert_eq!((p.receiver, p.hint), (None, vec!["random".to_owned(), "rng".to_owned()]));
        let op = |mut x: Expr| {
            recognize(&mut x, Language::Csharp);
            x.op
        };
        let random = Some("math.random".to_owned());
        // A field or local named like a generator, `new Random()`, `Random.Shared`.
        assert_eq!(op(call("_random", "Next", vec![e(Concept::Literal, "10", vec![])])), random);
        assert_eq!(op(call("rng", "NextDouble", vec![])), random);
        assert_eq!(op(call("sharedRandom", "Next", vec![])), random);
        let ctor = e(Concept::Construct, "new Random", vec![]);
        assert_eq!(op(e(Concept::Call, "Next", vec![target(ctor)])), random);
        let shared = e(Concept::GetMember, ".Shared", vec![e(Concept::Var, "Random", vec![])]);
        assert_eq!(op(e(Concept::Call, "Next", vec![target(shared), e(Concept::Literal, "1", vec![]), e(Concept::Literal, "6", vec![])])), random);
        // A state, an iterator, a linked list node: their own `Next`.
        assert_eq!(op(call("_state", "Next", vec![e(Concept::Var, "this", vec![])])), None);
        assert_eq!(op(call("node", "Next", vec![])), None);
        assert_eq!(op(call("brand", "Next", vec![])), None, "`brand` is not `rand`");
        // With semantics, the resolved target decides.
        let mut lib = call("source", "Next", vec![]);
        lib.target = Some("System.Random.Next(int)".into());
        assert_eq!(op(lib), random);
        let mut mine = call("random", "Next", vec![]);
        mine.target = Some("Game.RandomTable.Next()".into());
        assert_eq!(op(mine), None);
    }

    #[test]
    fn semantic_target_must_agree() {
        let mut x = call("Math", "Max", vec![]);
        x.target = Some("MyGame.Math.Max(int, int)".into());
        recognize(&mut x, Language::Csharp);
        // `MyGame.Math.Max` still ends with `Math.Max`: the receiver path is all we can check.
        assert_eq!(x.op.as_deref(), Some("math.max"));
        // `this.http.get(url)` resolved to Angular's HttpClient is not a map lookup.
        let mut get = call("http", "get", vec![]);
        get.target = Some("HttpClient.get(string)".into());
        recognize(&mut get, Language::Typescript);
        assert_eq!(get.op, None);
        let mut push = call("items", "push", vec![]);
        push.target = Some("Array.push(...T[])".into());
        recognize(&mut push, Language::Typescript);
        assert_eq!(push.op.as_deref(), Some("list.add"));
        let mut y = call("Math", "Max", vec![]);
        y.target = Some("Utils.Clamp(int, int)".into());
        recognize(&mut y, Language::Csharp);
        assert_eq!(y.op, None);
    }
}
