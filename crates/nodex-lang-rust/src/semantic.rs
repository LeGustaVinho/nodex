//! Semantics from rust-analyzer: `rust-analyzer scip` writes a SCIP index of the project
//! (every name with what it refers to), which becomes [`SemanticFacts`] keyed by byte ranges
//! (ADR-009):
//! - a reference to a function or method in call position → `Call` (site: the call);
//! - a reference to a struct or enum variant in a struct literal or tuple constructor →
//!   `Construct`;
//! - the definition of a local or parameter with its type (`let mut total: i64`) → local type.
//!
//! Each analysis re-runs the indexer (seconds on small crates; rust-analyzer loads the standard
//! library first). Install it with `rustup component add rust-analyzer`, or point
//! `NODEX_RUST_ANALYZER` at a binary.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use nodex_ir::semantic::{ByteRange, ExpressionFact, ExpressionFactKind, LocalTypeFact, RefKind, RustNominalType, SemanticFacts, SemanticRef, SemanticTarget, TypeFact, ValueFact, ValueFactTarget};
use nodex_ir::reading::{ReadingContent, ReadingFact};
use nodex_ir::types::{CollectionKind, TypeInfo, TypeOrigin, TypeShape, native_shape};
use nodex_ir::{Language, ValueBasis, ValueInfo, ValueKind};
use nodex_lang_api::blueprint_types::{expression_type, project_pattern};
use nodex_lang_api::kit::LineIndex;
use nodex_lang_api::syntax::{Src, named_children};
use nodex_lang_api::SemanticProvider;
use protobuf::Message;
use tree_sitter::Node;

use crate::reading::{
    DefaultCollection, DefaultType, Evidence, Family, NumericAlias, Operations, Resolved,
    TypeIdentities, may_shadow_cast_type, written_default_collection,
};

/// Semantic facts from `rust-analyzer scip`.
pub struct RustAnalyzer {
    root: PathBuf,
    binary: PathBuf,
    version: String,
    /// Folders indexed on their own (the project root, or each top-level Cargo package).
    crate_roots: Vec<PathBuf>,
    /// Package names of the project (their symbols are not external).
    local_packages: HashSet<String>,
}

/// The rust-analyzer binary: `NODEX_RUST_ANALYZER`, the rustup component, or `PATH`.
pub fn find_binary() -> Option<(PathBuf, String)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("NODEX_RUST_ANALYZER") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(out) = nodex_lang_api::process::command("rustup").args(["which", "rust-analyzer"]).output()
        && out.status.success()
    {
        candidates.push(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
    }
    candidates.push(PathBuf::from("rust-analyzer"));
    for c in candidates {
        if let Ok(out) = nodex_lang_api::process::command(&c).arg("--version").output()
            && out.status.success()
        {
            return Some((c, String::from_utf8_lossy(&out.stdout).trim().to_owned()));
        }
    }
    None
}

impl RustAnalyzer {
    pub fn start(root: &Path) -> Result<Self, String> {
        let (binary, version) =
            find_binary().ok_or("rust-analyzer was not found: install it with `rustup component add rust-analyzer` (or set NODEX_RUST_ANALYZER)")?;
        let packages = crate::project::cargo_packages(root);
        let local_packages = packages.iter().map(|p| p.name.clone()).collect();
        let crate_roots = if root.join("Cargo.toml").is_file() {
            vec![root.to_owned()]
        } else {
            // Separate packages without a workspace: index each top-level one.
            let mut dirs: Vec<PathBuf> = packages.iter().filter_map(|p| p.manifest.parent().map(Path::to_path_buf)).collect();
            dirs.sort_by_key(|d| d.components().count());
            let mut kept: Vec<PathBuf> = Vec::new();
            for d in dirs {
                if !kept.iter().any(|k| d.starts_with(k)) {
                    kept.push(d);
                }
            }
            kept
        };
        if crate_roots.is_empty() {
            return Err("no Cargo.toml found: rust-analyzer needs a Cargo package".into());
        }
        Ok(Self { root: root.to_owned(), binary, version, crate_roots, local_packages })
    }

    fn index(&self, crate_root: &Path, n: usize) -> Result<scip::types::Index, String> {
        let out_dir = self.root.join(".nodex").join("semantic");
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let out = out_dir.join(format!("rust-{n}.scip"));
        let result = nodex_lang_api::process::command(&self.binary)
            .arg("scip")
            .arg(crate_root)
            .arg("--output")
            .arg(&out)
            .output()
            .map_err(|e| format!("could not run rust-analyzer: {e}"))?;
        if !result.status.success() {
            let err = String::from_utf8_lossy(&result.stderr);
            return Err(format!("rust-analyzer scip failed: {}", err.lines().rev().take(3).collect::<Vec<_>>().join(" | ")));
        }
        let bytes = std::fs::read(&out).map_err(|e| e.to_string())?;
        scip::types::Index::parse_from_bytes(&bytes).map_err(|e| format!("unreadable SCIP index: {e}"))
    }

    fn facts(&self) -> Result<SemanticFacts, String> {
        let mut facts = SemanticFacts {
            source: format!("{} (SCIP)", self.version),
            ..Default::default()
        };
        let packages = crate::project::cargo_packages(&self.root);
        for (n, crate_root) in self.crate_roots.iter().enumerate() {
            let index = self.index(crate_root, n)?;
            let prefix = crate_root.strip_prefix(&self.root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            let upstream_indexmaps = upstream_indexmap_versions(crate_root);
            convert(&index, &self.root, &prefix, &self.local_packages, &packages, &upstream_indexmaps, &mut facts);
        }
        Ok(facts)
    }
}

impl SemanticProvider for RustAnalyzer {
    fn describe(&self) -> String {
        self.version.clone()
    }

    fn analyze(&mut self) -> Result<SemanticFacts, String> {
        self.facts()
    }

    fn update(&mut self, _changed: &[PathBuf]) -> Result<SemanticFacts, String> {
        self.facts()
    }
}

/// A parsed SCIP symbol: `rust-analyzer cargo <package> <version> <descriptors>`.
struct Sym<'s> {
    package: &'s str,
    version: &'s str,
    descriptors: &'s str,
}

impl Sym<'_> {
    fn standard(&self) -> bool {
        matches!(self.package, "alloc" | "core" | "std") && self.version.starts_with("https://github.com/rust-lang/rust/")
    }
}

fn parse_symbol(symbol: &str) -> Option<Sym<'_>> {
    if symbol.starts_with("local ") {
        return None;
    }
    let mut parts = symbol.splitn(5, ' ');
    let (_scheme, _manager, package, version, descriptors) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    Some(Sym { package, version, descriptors })
}

/// `vec/Vec#push().` → `["vec", "Vec", "push()"]`; `impl#[`Foo<T>`][Display]fmt().` → `["Foo", "fmt()"]`.
fn segments(descriptors: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = descriptors.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' => {
                for d in chars.by_ref() {
                    if d == '`' {
                        break;
                    }
                    cur.push(d);
                }
            }
            '[' => {
                // `impl#[Type][Trait]`: keep the type only.
                let mut inner = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    if d != '`' {
                        inner.push(d);
                    }
                }
                // `impl#` was just closed as a segment: the first bracket names the type.
                if cur.is_empty() && out.last().is_some_and(|l| l == "impl") {
                    out.pop();
                    out.push(inner.split('<').next().unwrap_or(&inner).trim().to_owned());
                }
            }
            '/' | '#' | '.' | ':' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '(' => {
                cur.push_str("()");
                // Skip a disambiguator `(+1)`.
                for d in chars.by_ref() {
                    if d == ')' {
                        break;
                    }
                }
            }
            '!' => cur.push('!'),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn display(sym: &Sym) -> String {
    let krate = sym.package.replace('-', "_");
    std::iter::once(krate).chain(segments(sym.descriptors)).collect::<Vec<_>>().join("::")
}

fn kind_of(descriptors: &str) -> &'static str {
    if descriptors.ends_with("().") {
        if descriptors.contains('#') || descriptors.contains("impl") { "method" } else { "function" }
    } else if descriptors.ends_with('#') {
        "type"
    } else {
        "other"
    }
}

/// Byte offset of a SCIP position (0-based line, UTF-8 column).
fn byte_at(line_starts: &[usize], line: i32, col: i32) -> Option<usize> {
    line_starts.get(usize::try_from(line).ok()?).map(|s| s + usize::try_from(col).unwrap_or(0))
}

fn range_bytes(line_starts: &[usize], r: &[i32]) -> Option<(usize, usize)> {
    match r {
        [l, c1, c2] => Some((byte_at(line_starts, *l, *c1)?, byte_at(line_starts, *l, *c2)?)),
        [l1, c1, l2, c2] => Some((byte_at(line_starts, *l1, *c1)?, byte_at(line_starts, *l2, *c2)?)),
        _ => None,
    }
}

/// The call whose callee is `name` (`f(…)`, `a::f(…)`, `x.f(…)`, `x.f::<T>(…)`).
fn enclosing_call<'t>(name: Node<'t>) -> Option<Node<'t>> {
    let mut callee = name;
    let parent = name.parent()?;
    match parent.kind() {
        "scoped_identifier" if parent.child_by_field_name("name").is_some_and(|n| n.id() == name.id()) => callee = parent,
        "field_expression" if parent.child_by_field_name("field").is_some_and(|n| n.id() == name.id()) => callee = parent,
        _ => {}
    }
    if let Some(g) = callee.parent().filter(|g| g.kind() == "generic_function") {
        callee = g;
    }
    let call = callee.parent().filter(|c| c.kind() == "call_expression")?;
    call.child_by_field_name("function").is_some_and(|f| f.id() == callee.id()).then_some(call)
}

fn primitive_number(name: &str) -> Option<&'static str> {
    const NUMBERS: &[&str] = &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f16", "f32", "f64", "f128",
    ];
    NUMBERS.iter().copied().find(|number| *number == name)
}

fn numeric_conversion(descriptor: &str) -> Option<(Family, String)> {
    let implementation = descriptor.strip_prefix("convert/num/")?;
    let implementation = implementation.strip_prefix("ptr_try_from_impls/").unwrap_or(implementation);
    let (target, conversion) = implementation.strip_prefix("impl#[")?.split_once("][`")?;
    let (conversion, method) = conversion.split_once(">`]")?;
    let (trait_name, source) = conversion.split_once('<')?;
    let source = primitive_number(source)?;
    let target = primitive_number(target)?;
    match (trait_name, method) {
        ("From", "from().") => Some((Family::NumericFrom { source, target }, "from".into())),
        ("TryFrom", "try_from().") if !source.starts_with('f') && !target.starts_with('f') => {
            Some((Family::NumericTryFrom { source, target }, "try_from".into()))
        }
        _ => None,
    }
}

fn arithmetic_result(sym: &Sym<'_>, operator: &str) -> Option<TypeShape> {
    if !sym.standard() || sym.package != "core" {
        return None;
    }
    let (trait_name, method_name) = match operator {
        "+" => ("Add", "add()."),
        "-" => ("Sub", "sub()."),
        "*" => ("Mul", "mul()."),
        "/" => ("Div", "div()."),
        "%" => ("Rem", "rem()."),
        _ => return None,
    };
    let implementation = sym.descriptors.strip_prefix("ops/arith/impl#[")?;
    let (left, descriptor) = implementation.split_once("][`")?;
    let (right, method) = descriptor.strip_prefix(trait_name)?.strip_prefix('<')?.split_once(">`]")?;
    let left = left.strip_prefix("`&").and_then(|left| left.strip_suffix('`')).unwrap_or(left);
    let left = primitive_number(left)?;
    let right = right.strip_prefix('&').unwrap_or(right);
    // The primitive implementation and its shared-reference variants have the same output.
    (method == method_name && (right == "Self" || right == left))
        .then(|| native_shape(Language::Rust, left))
}

fn reading_operation(sym: &Sym<'_>, path: &str, external: bool) -> Option<(Family, String)> {
    let descriptor = sym.descriptors;
    let name = path.rsplit("::").next()?;
    let name = name.strip_suffix("()").unwrap_or(name);
    let family = if sym.standard() {
        match path {
            "alloc::macros::vec!" | "alloc::vec!" | "std::vec!" => return Some((Family::Macro, "vec".into())),
            "alloc::macros::format!" | "alloc::format!" | "std::format!" => return Some((Family::Macro, "format".into())),
            "core::macros::builtin::assert!" | "core::assert!" => return Some((Family::Macro, "assert".into())),
            "core::macros::matches!" => return Some((Family::Macro, "matches".into())),
            "core::option::Option::Some" => return Some((Family::Option, "Some".into())),
            "core::result::Result::Ok" => return Some((Family::Result, "Ok".into())),
            _ => {}
        }
        if !descriptor.ends_with("().") {
            return None;
        }
        if sym.package == "alloc" && descriptor == "collections/vec_deque/impl#[`VecDeque<T, A>`]pop_front()." {
            return Some((Family::Effect, "deque_pop_front".into()));
        }
        if sym.package == "alloc" && descriptor == "string/impl#[String]insert()." {
            return Some((Family::StringInsert, name.into()));
        }
        if sym.package == "alloc" && descriptor == "vec/impl#[`Vec<T, A>`]drain()." {
            return Some((Family::VecDrain, name.into()));
        }
        if sym.package == "alloc" && descriptor == "string/impl#[String]len()." {
            return Some((Family::Length, "bytes".into()));
        }
        if sym.package == "alloc" && descriptor == "vec/impl#[`Vec<T, A>`]len()." {
            return Some((Family::Length, "elements".into()));
        }
        if sym.package == "alloc" && matches!(descriptor,
            "vec/impl#[`Vec<T, A>`]retain()." | "vec/impl#[`Vec<T, A>`]retain_mut().")
        {
            return Some((Family::VecRetain, name.into()));
        }
        if sym.package == "core" {
            match descriptor {
                "array/from_fn()." => return Some((Family::ArrayFromFn, name.into())),
                "iter/sources/from_fn/from_fn()." => return Some((Family::IterFromFn, name.into())),
                "mem/take()." | "mem/replace()." => return Some((Family::MemReplace, name.into())),
                "num/nonzero/impl#[`NonZero<T>`]get()." => return Some((Family::NonZero, name.into())),
                "ops/range/impl#[`RangeInclusive<Idx>`]contains()." => return Some((Family::InclusiveRange, name.into())),
                "str/impl#[str]split()." | "str/impl#[str]split_once()." => return Some((Family::StrSplit, name.into())),
                "iter/adapters/peekable/impl#[`Peekable<I>`]peek()."
                | "iter/adapters/peekable/impl#[`Peekable<I>`]next_if()." => return Some((Family::Peekable, name.into())),
                "str/impl#[str]len()." => return Some((Family::Length, "bytes".into())),
                "slice/impl#[`[T]`]len()." => return Some((Family::Length, "elements".into())),
                _ => {}
            }
        }
        if sym.package == "core" && let Some(conversion) = numeric_conversion(descriptor) {
            return Some(conversion);
        }
        if sym.package == "core"
            && let Some(method) = descriptor.strip_prefix("slice/impl#[`[T]`]").and_then(|name| name.strip_suffix("()."))
        {
            let family = if matches!(method, "iter" | "iter_mut") { Family::Iter } else { Family::Slice };
            return Some((family, method.into()));
        }
        if sym.package == "std" && matches!(descriptor,
            "sync/once_lock/impl#[`OnceLock<T>`]take()."
            | "sync/once_lock/impl#[`OnceLock<T>`]get_mut()."
            | "sync/once_lock/impl#[`OnceLock<T>`]get_or_init().")
        {
            Family::OnceLock
        } else if sym.package == "core" && matches!(descriptor, "bool/impl#[bool]then()." | "bool/impl#[bool]then_some().") {
            Family::Bool
        } else if sym.package == "alloc" && (matches!(descriptor,
            "sync/impl#[`Arc<T, A>`]make_mut()." | "sync/impl#[`Arc<T, A>`]unwrap_or_clone().")
            || descriptor.starts_with("sync/impl#[`Arc<") && name == "clone")
        {
            Family::Arc
        } else if name == "clone" && (descriptor == "clone/Clone#clone()." || descriptor.contains("][Clone]clone().")) {
            Family::Clone
        } else if path.starts_with("core::option::Option::") {
            Family::Option
        } else if path.starts_with("core::result::Result::") {
            Family::Result
        } else if path.starts_with("std::collections::hash::map::HashMap::") || path.starts_with("alloc::collections::btree::map::BTreeMap::")
            || path.starts_with("std::collections::hash::map::Entry::") || path.starts_with("alloc::collections::btree::map::entry::Entry::")
        {
            Family::Map
        } else if path.starts_with("core::iter::traits::iterator::Iterator::")
                && (name != "zip" || sym.package == "core" && descriptor == "iter/traits/iterator/Iterator#zip().")
            || path.starts_with("core::iter::traits::collect::IntoIterator::")
            || name == "into_iter" && descriptor.contains("][IntoIterator]into_iter().")
            || descriptor.rsplit_once("][Iterator]").is_some_and(|(_, method)| {
                matches!(method, "find_map()." | "filter_map()." | "try_fold().")
            })
            || sym.package == "core" && matches!(descriptor,
                "slice/iter/impl#[`Iter<'a, T>`][Iterator]find()."
                | "slice/iter/impl#[`Iter<'a, T>`][Iterator]position()."
                | "slice/iter/impl#[`Iter<'a, T>`][Iterator]any()."
                | "slice/iter/impl#[`Iter<'a, T>`][Iterator]all()."
                | "slice/iter/impl#[`IterMut<'a, T>`][Iterator]find()."
                | "slice/iter/impl#[`IterMut<'a, T>`][Iterator]position()."
                | "slice/iter/impl#[`IterMut<'a, T>`][Iterator]any()."
                | "slice/iter/impl#[`IterMut<'a, T>`][Iterator]all()."
                | "str/iter/impl#[`Bytes<'_>`][Iterator]any()."
                | "str/iter/impl#[`Bytes<'_>`][Iterator]all()."
                | "iter/adapters/rev/impl#[`Rev<I>`][Iterator]find()."
                | "iter/adapters/chain/impl#[`Chain<A, B>`][Iterator]find().")
            || matches!(name, "iter" | "iter_mut") && matches!(sym.package, "alloc" | "std") && descriptor.starts_with("collections/")
        {
            Family::Iter
        } else if sym.package == "std" && descriptor.starts_with("thread/")
            && (name == "spawn" || name == "join" && path.contains("::JoinHandle::"))
        {
            Family::Thread
        } else if sym.package == "std" && descriptor.starts_with("sync/")
            && (path.contains("::Mutex::") || path.contains("::RwLock::"))
        {
            Family::Lock
        } else if sym.package == "core" && path == "core::cell::RefCell::borrow_mut()" {
            Family::Cell
        } else if matches!(path, "core::mem::drop()" | "std::process::exit()")
            || sym.package == "std" && (descriptor.starts_with("thread/") && name == "sleep"
                || descriptor.starts_with("sync/mpsc/") && matches!(name, "send" | "recv"))
        {
            Family::Effect
        } else {
            return None;
        }
    } else {
        match (external, sym.package, descriptor) {
            (true, "tokio", "task/spawn/spawn().") => Family::Task,
            (true, "tokio", "macros/select/select!" | "select!") => return Some((Family::Macro, "select".into())),
            (true, "tokio", descriptor) if (descriptor.starts_with("time/") && matches!(name, "sleep" | "timeout_at"))
                || (descriptor.starts_with("sync/mpsc/") || descriptor.starts_with("sync/broadcast/") || descriptor.starts_with("sync/oneshot/"))
                    && matches!(name, "send" | "recv")
                || descriptor.starts_with("sync/semaphore/") && name == "acquire" => Family::Effect,
            (true, "rayon", descriptor) if descriptor.starts_with("iter/")
                && (descriptor.contains("ParallelIterator#") || descriptor.contains("][`IntoParallelRefIterator")
                    || descriptor.contains("][`IntoParallelRefMutIterator") || descriptor.contains("][IntoParallelIterator]")) => Family::ParIter,
            _ => return None,
        }
    };
    let operation = match (family, sym.package, name) {
        (Family::Effect, "std", "recv") if descriptor == "sync/mpsc/impl#[`Receiver<T>`]recv()." => "recv_blocking",
        (Family::Effect, "tokio", "recv") if descriptor.starts_with("sync/mpsc/") => "recv_optional",
        (Family::Effect, "tokio", "send") if descriptor.starts_with("sync/mpsc/bounded/") => "send_async",
        (Family::Effect, "tokio", "sleep") => "sleep_async",
        _ => name,
    };
    Some((family, operation.into()))
}

fn record_operation(operations: &mut Operations, node: Node<'_>, resolved: Resolved) {
    operations.entry((node.start_byte(), node.end_byte())).and_modify(|previous| {
        if previous.as_ref().is_some_and(|op| op.family != resolved.family || op.name != resolved.name || op.definition != resolved.definition) {
            *previous = None;
        }
    }).or_insert_with(|| Some(resolved));
}

fn macro_invocation(name: Node<'_>) -> Option<Node<'_>> {
    let invocation = std::iter::successors(Some(name), Node::parent).find(|node| node.kind() == "macro_invocation")?;
    let callee = invocation.child_by_field_name("macro")?;
    (callee.start_byte() <= name.start_byte() && name.end_byte() <= callee.end_byte()).then_some(invocation)
}

/// The struct literal whose type is `name` (`Order { … }`, `Shape::Circle { … }`).
fn enclosing_struct_literal<'t>(name: Node<'t>) -> Option<Node<'t>> {
    let mut n = name;
    while let Some(p) = n.parent() {
        match p.kind() {
            "scoped_type_identifier" | "scoped_identifier" | "generic_type" => n = p,
            "struct_expression" => return p.child_by_field_name("name").is_some_and(|x| x.id() == n.id()).then_some(p),
            _ => return None,
        }
    }
    None
}

/// `let mut total: i64` → (`total`, `i64`); `customer: &str` → (`customer`, `&str`).
fn local_signature(text: &str) -> Option<(String, String)> {
    let t = text.trim().trim_start_matches("let ").trim_start_matches("mut ").trim_start_matches("ref ");
    let (name, ty) = t.split_once(": ")?;
    let name = name.trim();
    if name == "self" || name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((name.to_owned(), ty.trim().to_owned()))
}

fn named_value_site(node: Node<'_>) -> Option<(Node<'_>, ValueFactTarget)> {
    let target = match node.kind() {
        "field_declaration" => ValueFactTarget::Member,
        "parameter" | "let_declaration" => ValueFactTarget::Binding,
        _ => return None,
    };
    let mut name = node.child_by_field_name("name").or_else(|| node.child_by_field_name("pattern"))?;
    if name.kind() == "mut_pattern" {
        name = named_children(name).into_iter().last()?;
    }
    (name.kind() == "identifier" || name.kind() == "field_identifier").then_some((name, target))
}

fn outer_type_declaration(mut name: Node<'_>) -> Option<(Node<'_>, bool)> {
    let mut by_reference = false;
    while let Some(parent) = name.parent() {
        let field = match parent.kind() {
            "generic_type" => "type",
            "scoped_type_identifier" => "name",
            "tuple_type" if crate::syntax::unparen_type(parent).id() == crate::syntax::unparen_type(name).id() => {
                name = parent;
                continue;
            }
            "reference_type" if !by_reference => {
                by_reference = true;
                "type"
            }
            _ => return parent.child_by_field_name("type").filter(|ty| ty.id() == name.id()).map(|_| (parent, by_reference)),
        };
        if !parent.child_by_field_name(field).is_some_and(|part| part.id() == name.id()) {
            return None;
        }
        name = parent;
    }
    None
}

/// A constructor fixes the outer type of an unannotated local, including a
/// direct borrow of the constructor. Explicit annotations may coerce it.
fn constructor_type_sites(mut value: Node<'_>) -> Vec<(Node<'_>, bool)> {
    let mut sites = vec![(value, false)];
    let mut by_reference = false;
    while let Some(parent) = value.parent() {
        match parent.kind() {
            "parenthesized_expression" => {}
            "reference_expression" if !by_reference
                && !nodex_lang_api::syntax::children(parent).iter().any(|child| child.kind() == "raw")
                && parent.child_by_field_name("value").is_some_and(|operand| operand.id() == value.id()) =>
            {
                by_reference = true;
                sites.push((parent, true));
            }
            "let_declaration" if parent.child_by_field_name("type").is_none()
                && parent.child_by_field_name("value").is_some_and(|initializer| initializer.id() == value.id()) =>
            {
                if let Some((binding, _)) = named_value_site(parent) {
                    sites.push((binding, by_reference));
                }
                break;
            }
            _ => break,
        }
        value = parent;
    }
    sites
}

fn propagate_nominal_locals(
    facts: &mut Vec<RustNominalType>,
    first: usize,
    file: &str,
    root: Node<'_>,
    definitions: &HashMap<&str, (usize, usize)>,
    uses: &[(usize, usize, &str)],
) {
    let mut types = HashMap::new();
    let record = |fact: &RustNominalType, types: &mut HashMap<_, _>| {
        let evidence = (fact.declaration.clone(), fact.by_reference);
        types.entry((fact.site.start, fact.site.end)).and_modify(|previous: &mut Option<_>| {
            if previous.as_ref() != Some(&evidence) {
                *previous = None;
            }
        }).or_insert(Some(evidence));
    };
    for fact in &facts[first..] {
        record(fact, &mut types);
    }
    for &(start, end, symbol) in uses {
        let Some(&(decl_start, decl_end)) = definitions.get(symbol) else { continue };
        let Some(Some((declaration, by_reference))) = types.get(&(decl_start as u32, decl_end as u32)).cloned() else { continue };
        let Some(name) = root.descendant_for_byte_range(start, end).filter(|name| name.kind() == "identifier") else { continue };
        for (node, borrow) in constructor_type_sites(name).into_iter().skip(1) {
            if borrow && by_reference {
                continue;
            }
            let fact = RustNominalType {
                site: ByteRange { file: file.to_owned(), start: node.start_byte() as u32, end: node.end_byte() as u32 },
                declaration: declaration.clone(),
                by_reference: by_reference || borrow,
            };
            record(&fact, &mut types);
            facts.push(fact);
        }
    }
}

fn local_nominal_type(name: Node<'_>) -> bool {
    let Some(decl) = name.parent() else { return false };
    matches!(decl.kind(), "struct_item" | "enum_item" | "union_item") &&
        decl.child_by_field_name("name").is_some_and(|id| id.id() == name.id())
}

type Range = (usize, usize);

struct ReturnType {
    info: TypeInfo,
    cast_alias: Option<NumericAlias>,
    default_collection: Option<DefaultCollection>,
    generics: Vec<Option<String>>,
    parameters: Vec<Option<TypeShape>>,
    receiver: bool,
    owner: Option<TypeShape>,
    owner_generics: Vec<String>,
}

#[derive(Default)]
struct InitializerTypes<'a> {
    shapes: HashMap<Range, TypeShape>,
    calls: HashMap<Range, &'a ReturnType>,
}

/// SCIP identifies the tokens in written types, while signatures of locals use short names.
fn written_shape(ty: Node<'_>, text: &str, identities: &TypeIdentities) -> TypeShape {
    let mut resolved = String::new();
    let mut start = ty.start_byte();
    for (&at, (end, path)) in identities.range(ty.byte_range()) {
        if at < start || *end > ty.end_byte() { continue; }
        resolved.push_str(&text[start..at]);
        resolved.push_str(path);
        start = *end;
    }
    resolved.push_str(&text[start..ty.end_byte()]);
    native_shape(Language::Rust, &resolved)
}

fn written_type_identities(doc: &scip::types::Document, starts: &[usize], root: Node<'_>, text: &str) -> TypeIdentities {
    doc.occurrences.iter().filter_map(|occurrence| {
        let symbol = parse_symbol(&occurrence.symbol)?;
        let primitive_module = symbol.standard() && symbol.package == "core" && symbol.descriptors == "primitive/";
        if !primitive_module && kind_of(symbol.descriptors) != "type" { return None; }
        let (start, end) = range_bytes(starts, &occurrence.range)?;
        let mut node = root.descendant_for_byte_range(start, end)?;
        if node.start_byte() != start || node.end_byte() != end { return None; }
        while let Some(parent) = node.parent().filter(|parent| {
            matches!(parent.kind(), "scoped_type_identifier" | "scoped_identifier")
                && parent.child_by_field_name("name").is_some_and(|name| name.id() == node.id())
        }) {
            node = parent;
        }
        if primitive_module {
            // SCIP resolves the standard primitive module. The final primitive has no occurrence.
            let ty = node.parent().filter(|parent| parent.kind() == "scoped_type_identifier"
                && parent.child_by_field_name("path") == Some(node))?;
            let name = &text[ty.child_by_field_name("name")?.byte_range()];
            return matches!(native_shape(Language::Rust, name), TypeShape::Scalar { .. })
                .then(|| (ty.start_byte(), (ty.end_byte(), name.to_owned())));
        }
        let path = display(&symbol);
        // A symbol on a primitive-shaped token identifies a shadowing type.
        let name = node.child_by_field_name("name").unwrap_or(node);
        if name.kind() != "primitive_type" && (!symbol.standard() || path != "core::option::Option"
            && !matches!(native_shape(Language::Rust, &path), TypeShape::Collection { .. } | TypeShape::Scalar { .. }))
        {
            return None;
        }
        Some((node.start_byte(), (node.end_byte(), path)))
    }).collect()
}

fn upstream_indexmap_versions(root: &Path) -> HashSet<String> {
    let Some(lockfile) = root.ancestors().map(|dir| dir.join("Cargo.lock")).find(|path| path.is_file()) else {
        return HashSet::new();
    };
    let Some(packages) = std::fs::read_to_string(lockfile).ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .and_then(|mut lock| lock.remove("package"))
    else { return HashSet::new() };
    let mut registry = HashSet::new();
    let mut other = HashSet::new();
    for package in packages.as_array().into_iter().flatten() {
        if package.get("name").and_then(toml::Value::as_str) != Some("indexmap") { continue; }
        let Some(version) = package.get("version").and_then(toml::Value::as_str) else { continue };
        if package.get("source").and_then(toml::Value::as_str) == Some("registry+https://github.com/rust-lang/crates.io-index") {
            registry.insert(version.to_owned());
        } else {
            other.insert(version);
        }
    }
    registry.retain(|version| !other.contains(version.as_str()));
    registry
}

fn default_type_identities(
    doc: &scip::types::Document, starts: &[usize], root: Node<'_>,
    upstream_indexmaps: &HashSet<String>,
) -> HashMap<Range, DefaultType> {
    let mut sites = HashMap::new();
    for occurrence in &doc.occurrences {
        let Some(symbol) = parse_symbol(&occurrence.symbol) else { continue };
        if !symbol.descriptors.ends_with('#') { continue; }
        let Some((start, end)) = range_bytes(starts, &occurrence.range) else { continue };
        let Some(leaf) = root.descendant_for_byte_range(start, end)
            .filter(|node| node.start_byte() == start && node.end_byte() == end)
        else { continue };
        let name = leaf.parent().filter(|parent| parent.kind() == "scoped_type_identifier"
            && parent.child_by_field_name("name") == Some(leaf)).unwrap_or(leaf);
        if !name.parent().is_some_and(|parent| parent.kind() == "generic_type"
            && parent.child_by_field_name("type") == Some(name))
        {
            continue;
        }
        let kind = match (symbol.package, symbol.descriptors) {
            ("alloc", "vec/Vec#") if symbol.standard() => Some(DefaultType::Vec),
            ("core", "option/Option#") if symbol.standard() => Some(DefaultType::Option),
            ("core", "result/Result#") if symbol.standard() => Some(DefaultType::Result),
            ("indexmap", "map/IndexMap#") if symbol.version.starts_with("2.") && upstream_indexmaps.contains(symbol.version) => Some(DefaultType::IndexMap),
            _ => None,
        };
        record_unambiguous(&mut sites, (name.start_byte(), name.end_byte()), kind);
    }
    sites.into_iter().filter_map(|(range, kind)| Some((range, kind.flatten()?))).collect()
}

type CastAliases = HashMap<Range, NumericAlias>;

fn record_unambiguous<K: std::hash::Hash + Eq, V: Eq>(
    items: &mut HashMap<K, Option<V>>,
    key: K,
    value: V,
) {
    items
        .entry(key)
        .and_modify(|previous| {
            if previous.as_ref() != Some(&value) {
                *previous = None;
            }
        })
        .or_insert(Some(value));
}

fn conditional_alias(alias: Node<'_>, text: &str) -> bool {
    let is_conditional = |attribute: Node<'_>| {
        named_children(attribute)
            .into_iter()
            .find(|child| child.kind() == "attribute")
            .and_then(|attribute| {
                named_children(attribute)
                    .into_iter()
                    .find(|child| !child.is_extra())
            })
            .is_some_and(|name| matches!(&text[name.byte_range()], "cfg" | "cfg_attr"))
    };
    std::iter::successors(Some(alias), Node::parent).any(|scope| {
        std::iter::successors(scope.prev_named_sibling(), Node::prev_named_sibling)
            .take_while(|previous| previous.is_extra() || previous.kind() == "attribute_item")
            .filter(|previous| previous.kind() == "attribute_item")
            .any(is_conditional)
            || named_children(scope)
                .into_iter()
                .filter(|child| child.kind() == "inner_attribute_item")
                .any(is_conditional)
    })
}

fn resolved_cast_aliases(
    index: &scip::types::Index,
    rel_of: impl Fn(&scip::types::Document) -> String,
    texts: &HashMap<String, (String, Vec<usize>)>,
    typed_documents: &HashMap<String, (tree_sitter::Tree, TypeIdentities)>,
) -> HashMap<String, CastAliases> {
    enum AliasTarget<'a> {
        Primitive(&'static str),
        Alias(&'a str),
    }

    let mut definitions = HashMap::new();
    let mut uses = HashMap::new();
    for doc in &index.documents {
        let file = rel_of(doc);
        let Some((_, starts)) = texts.get(&file) else {
            continue;
        };
        let Some((tree, _)) = typed_documents.get(&file) else {
            continue;
        };
        let mut sites = HashMap::new();
        for occurrence in &doc.occurrences {
            let symbol = occurrence.symbol.as_str();
            if !symbol.ends_with('#') && !symbol.starts_with("local ") {
                continue;
            }
            let Some((start, end)) = range_bytes(starts, &occurrence.range) else {
                continue;
            };
            let Some(leaf) = tree
                .root_node()
                .descendant_for_byte_range(start, end)
                .filter(|leaf| leaf.start_byte() == start && leaf.end_byte() == end)
            else {
                continue;
            };
            if occurrence.symbol_roles & 1 == 1 {
                if symbol.ends_with('#') {
                    record_unambiguous(&mut definitions, symbol, (file.clone(), (start, end)));
                }
            } else if matches!(leaf.kind(), "type_identifier" | "primitive_type") {
                let written = leaf
                    .parent()
                    .filter(|parent| {
                        parent.kind() == "scoped_type_identifier"
                            && parent.child_by_field_name("name") == Some(leaf)
                    })
                    .unwrap_or(leaf);
                record_unambiguous(
                    &mut sites,
                    (written.start_byte(), written.end_byte()),
                    symbol,
                );
            }
        }
        uses.insert(file, sites);
    }

    let mut line_indexes = HashMap::new();
    let targets: HashMap<_, _> = definitions
        .iter()
        .filter_map(|(&symbol, definition)| {
            let (file, (start, end)) = definition.as_ref()?;
            let (text, _) = texts.get(file)?;
            let (tree, identities) = typed_documents.get(file)?;
            let name = tree.root_node().descendant_for_byte_range(*start, *end)?;
            let item = name.parent().filter(|parent| {
                parent.kind() == "type_item" && parent.child_by_field_name("name") == Some(name)
            })?;
            if item.has_error()
                || item.child_by_field_name("type_parameters").is_some()
                || item
                    .parent()
                    .and_then(|parent| parent.parent())
                    .is_some_and(|owner| matches!(owner.kind(), "impl_item" | "trait_item"))
                || conditional_alias(item, text)
            {
                return None;
            }
            let ty = crate::syntax::unparen_type(item.child_by_field_name("type")?);
            if !matches!(
                ty.kind(),
                "primitive_type" | "type_identifier" | "scoped_type_identifier"
            ) {
                return None;
            }
            if let Some(resolved) = uses.get(file)?.get(&(ty.start_byte(), ty.end_byte())) {
                return Some((symbol, AliasTarget::Alias((*resolved)?)));
            }
            let primitive = if let Some((_, identity)) = identities
                .get(&ty.start_byte())
                .filter(|(end, _)| *end == ty.end_byte())
            {
                primitive_number(identity)?
            } else {
                let index = line_indexes
                    .entry(file)
                    .or_insert_with(|| LineIndex::new(text));
                let src = Src { text, file, index };
                if ty.kind() != "primitive_type" || may_shadow_cast_type(src, ty, false) {
                    return None;
                }
                primitive_number(src.text(ty))?
            };
            Some((symbol, AliasTarget::Primitive(primitive)))
        })
        .collect();

    let resolved: HashMap<_, _> = targets
        .keys()
        .filter_map(|&symbol| {
            let mut next = symbol;
            for _ in 0..16 {
                match targets.get(next)? {
                    AliasTarget::Primitive(primitive) => return Some((symbol, *primitive)),
                    AliasTarget::Alias(alias) => next = alias,
                }
            }
            None
        })
        .collect();
    uses.into_iter()
        .map(|(file, sites)| {
            let text = &texts[&file].0;
            let aliases = sites
                .into_iter()
                .filter_map(|(range, symbol)| {
                    let primitive = *resolved.get(symbol?)?;
                    let spelling = text.get(range.0..range.1)?.trim().to_owned();
                    Some((
                        range,
                        NumericAlias {
                            primitive,
                            spelling,
                        },
                    ))
                })
                .collect();
            (file, aliases)
        })
        .collect()
}

fn with_type_identity(inferred: TypeShape, evidence: &TypeShape) -> TypeShape {
    let refine = |inferred: Vec<TypeShape>, hints: &[TypeShape]| {
        if inferred.is_empty() { return hints.to_vec(); }
        inferred.into_iter().enumerate().map(|(i, shape)| match hints.get(i) {
            Some(hint) => with_type_identity(shape, hint),
            None => shape,
        }).collect()
    };
    match (inferred, evidence) {
        (TypeShape::Unknown, known) => known.clone(),
        (TypeShape::Named { arguments, .. },
            TypeShape::Collection { family, arguments: hints, rank, length }) =>
            TypeShape::Collection { family: *family, arguments: refine(arguments, hints), rank: *rank, length: length.clone() },
        (TypeShape::Collection { family, arguments, rank, length },
            TypeShape::Collection { family: hint_family, arguments: hints, .. }) if family == *hint_family =>
            TypeShape::Collection { family, arguments: refine(arguments, hints), rank, length },
        (TypeShape::Named { name, arguments }, TypeShape::Named { name: written, arguments: hints }) if &name == written =>
            TypeShape::Named { name, arguments: refine(arguments, hints) },
        (TypeShape::Named { name, .. }, TypeShape::Scalar { name: known }) if &name == known =>
            TypeShape::scalar(known),
        (TypeShape::Tuple { items }, TypeShape::Tuple { items: hints }) =>
            TypeShape::Tuple { items: refine(items, hints) },
        (TypeShape::Indirect { pointer, target }, TypeShape::Indirect { target: hint, .. }) =>
            TypeShape::Indirect { pointer, target: Box::new(with_type_identity(*target, hint)) },
        (other, _) => other,
    }
}

fn infer_type_arguments(
    formal: &TypeShape, actual: &TypeShape, inferred: &mut HashMap<&str, Option<TypeShape>>,
) {
    if let TypeShape::Named { name, arguments } = formal
        && arguments.is_empty()
        && let Some(binding) = inferred.get_mut(name.as_str())
    {
        if *actual != TypeShape::Unknown {
            match binding {
                None => *binding = Some(actual.clone()),
                Some(previous) if previous != actual => *previous = TypeShape::Unknown,
                _ => {}
            }
        }
        return;
    }
    let (formals, actuals) = match (formal, actual) {
        (TypeShape::Named { name: a, arguments: formals }, TypeShape::Named { name: b, arguments: actuals }) if a == b =>
            (formals, actuals),
        (TypeShape::Collection { family: a, arguments: formals, .. },
            TypeShape::Collection { family: b, arguments: actuals, .. }) if a == b => (formals, actuals),
        (TypeShape::Tuple { items: formals }, TypeShape::Tuple { items: actuals }) => (formals, actuals),
        (TypeShape::Indirect { pointer: a, target: formal }, TypeShape::Indirect { pointer: b, target: actual }) if a == b => {
            infer_type_arguments(formal, actual, inferred);
            return;
        }
        _ => return,
    };
    for (formal, actual) in formals.iter().zip(actuals) {
        infer_type_arguments(formal, actual, inferred);
    }
}

fn substitute_type_arguments(shape: &mut TypeShape, types: &HashMap<&str, Option<TypeShape>>) {
    let children = match shape {
        TypeShape::Named { name, arguments } => {
            if arguments.is_empty() && let Some(ty) = types.get(name.as_str()) {
                *shape = ty.clone().unwrap_or(TypeShape::Unknown);
                return;
            }
            arguments
        }
        TypeShape::Collection { arguments, .. } => arguments,
        TypeShape::Tuple { items } | TypeShape::Union { items } => items,
        TypeShape::Indirect { target, .. } => {
            substitute_type_arguments(target, types);
            return;
        }
        _ => return,
    };
    for child in children {
        substitute_type_arguments(child, types);
    }
}

fn call_result<'tree>(
    template: &ReturnType, call: Node<'tree>, src: Src<'_>, identities: &TypeIdentities,
    argument_type: impl Fn(Node<'tree>) -> Option<TypeShape>,
) -> TypeShape {
    let mut types: HashMap<_, _> = template.generics.iter().flatten().chain(&template.owner_generics)
        .map(|name| (name.as_str(), None)).collect();
    let function = call.child_by_field_name("function");
    let explicit = function.and_then(|function| function.child_by_field_name("type_arguments"));
    let callee = function.and_then(|function| if function.kind() == "generic_function" {
        function.child_by_field_name("function")
    } else { Some(function) });
    let args: Vec<_> = call.child_by_field_name("arguments").map(named_children).unwrap_or_default()
        .into_iter().filter(|arg| !arg.is_extra()).collect();
    let method = template.receiver && callee.is_some_and(|callee| callee.kind() == "field_expression");
    if let Some(owner) = &template.owner {
        let receiver = if method {
            callee.and_then(|callee| callee.child_by_field_name("value"))
        } else {
            args.first().copied().filter(|_| template.receiver)
        };
        if let Some(actual) = receiver.and_then(&argument_type) {
            let mut target = &actual;
            while let TypeShape::Indirect { pointer: false, target: inner } = target {
                target = inner;
            }
            infer_type_arguments(owner, target, &mut types);
        }
        if !method && let Some(qualifier) = callee.and_then(|callee| callee.child_by_field_name("path")) {
            infer_type_arguments(owner, &written_shape(qualifier, src.text, identities), &mut types);
        }
    }
    for (formal, arg) in template.parameters.iter().skip(usize::from(method)).zip(args) {
        if let Some(formal) = formal && let Some(actual) = argument_type(arg) {
            infer_type_arguments(formal, &actual, &mut types);
        }
    }
    if let Some(owner) = &template.owner {
        let mut actual = owner.clone();
        substitute_type_arguments(&mut actual, &types);
        types.insert("Self", Some(actual));
    }
    if let Some(explicit) = explicit {
        let args = named_children(explicit).into_iter().filter(|arg| !arg.is_extra() && arg.kind() != "lifetime");
        for (name, arg) in template.generics.iter().zip(args) {
            if let Some(name) = name && src.text(arg) != "_" {
                types.insert(name, Some(written_shape(arg, src.text, identities)));
            }
        }
    }
    let mut shape = template.info.shape.clone();
    substitute_type_arguments(&mut shape, &types);
    shape
}

fn type_parameters(declaration: Node<'_>, text: &str) -> Vec<Option<String>> {
    declaration.child_by_field_name("type_parameters").map(named_children).unwrap_or_default()
        .into_iter().filter_map(|param| match param.kind() {
            "type_parameter" => Some(param.child_by_field_name("name").map(|name| text[name.byte_range()].to_owned())),
            "const_parameter" => Some(None),
            _ => None,
        }).collect()
}

/// A nested element must be a complete vec! invocation. Direct arguments share the
/// outer macro's lexical scope, even when SCIP only identifies the outer invocation.
fn vec_macro_shape(tokens: Node<'_>, name: &str, text: &str, resolved: &HashSet<Range>, depth: u8) -> TypeShape {
    use nodex_lang_api::syntax::children;
    let contents: Vec<_> = children(tokens).into_iter().filter(|child| !child.is_extra()).collect();
    let mut element = TypeShape::Unknown;
    if depth < 7 && contents.len() >= 2 {
        let arguments = &contents[1..contents.len() - 1];
        for argument in arguments.split(|node| matches!(node.kind(), "," | ";")) {
            let [path @ .., bang, nested] = argument else { continue };
            let (Some(first), Some(last)) = (path.first(), path.last()) else { continue };
            let spelling = &text[first.start_byte()..last.end_byte()];
            let resolved_path = resolved.contains(&(last.start_byte(), last.end_byte()))
                && path.iter().rev().enumerate().all(|(i, part)| if i % 2 == 0 {
                    matches!(part.kind(), "identifier" | "crate" | "self" | "super")
                } else { part.kind() == "::" });
            if bang.kind() == "!" && nested.kind() == "token_tree"
                && (spelling == name || resolved_path)
            {
                element = with_type_identity(element, &vec_macro_shape(*nested, spelling, text, resolved, depth + 1));
            }
        }
    }
    TypeShape::collection(CollectionKind::List, vec![element])
}

fn initializer_type(
    src: Src<'_>, value: Node<'_>, identities: &TypeIdentities, initializers: &InitializerTypes<'_>,
    local: &impl Fn(Range) -> Option<TypeShape>, depth: usize,
) -> Option<TypeShape> {
    if depth > 12 { return None; }
    let range = (value.start_byte(), value.end_byte());
    if let Some(shape) = initializers.shapes.get(&range).cloned().or_else(|| local(range)) {
        return Some(shape);
    }
    let child_type = |child| initializer_type(src, child, identities, initializers, local, depth + 1);
    if let Some(template) = initializers.calls.get(&range) {
        return Some(call_result(template, value, src, identities, child_type));
    }
    let operands = || named_children(value).into_iter().filter(|child| !child.is_extra());
    match value.kind() {
        "parenthesized_expression" | "negative_literal" => child_type(operands().next()?),
        "reference_expression" => {
            let operand = value.child_by_field_name("value")?;
            let pointer = nodex_lang_api::syntax::children(value).iter().any(|child| child.kind() == "raw");
            Some(TypeShape::Indirect { pointer, target: Box::new(child_type(operand)?) })
        }
        "tuple_expression" => Some(TypeShape::Tuple {
            items: operands().map(|child| child_type(child).unwrap_or(TypeShape::Unknown)).collect(),
        }),
        "array_expression" => {
            let TypeShape::Collection { family, rank, length, .. } = expression_type(Language::Rust, src, value)? else {
                return None;
            };
            let repeat = value.child_by_field_name("length");
            let element = TypeShape::union(operands().filter(|child| repeat != Some(*child))
                .map(|child| child_type(child).unwrap_or(TypeShape::Unknown)));
            Some(TypeShape::Collection { family, rank, length, arguments: vec![element] })
        }
        "integer_literal" | "float_literal" | "boolean_literal" | "char_literal" |
        "string_literal" | "raw_string_literal" => {
            let shape = expression_type(Language::Rust, src, value)?;
            (!matches!(&shape, TypeShape::Scalar { name } if matches!(name.as_str(), "Integer" | "Float"))).then_some(shape)
        }
        _ => None,
    }
}

fn rust_type_facts(
    facts: &mut SemanticFacts,
    file: &str,
    text: &str,
    root: Node<'_>,
    definitions: &HashMap<&str, (usize, usize)>,
    uses: &[(usize, usize, &str)],
    signatures: &HashMap<&str, &str>,
    identities: &TypeIdentities,
    initializers: &InitializerTypes<'_>,
) {
    let index = LineIndex::new(text);
    let src = Src { text, file, index: &index };
    let site = |start: usize, end: usize| ByteRange { file: file.into(), start: start as u32, end: end as u32 };
    let mut types: HashMap<_, TypeInfo> = HashMap::new();
    let uses_by_site: HashMap<_, _> = uses.iter().map(|&(start, end, symbol)| ((start, end), symbol)).collect();
    let initializer = |value, types: &HashMap<&str, TypeInfo>| {
        initializer_type(src, value, identities, initializers, &|range| {
            types.get(uses_by_site.get(&range)?).map(|info| info.shape.clone())
        }, 0)
    };
    let mut bindings: Vec<_> = definitions.iter().collect();
    bindings.sort_by_key(|(_, (start, _))| *start);
    let mut typed_initializers = HashSet::new();
    for (&symbol, &(start, end)) in bindings {
        let Some((_, ty)) = signatures.get(symbol).and_then(|sig| local_signature(sig)) else { continue };
        let leaf = root.descendant_for_byte_range(start, end);
        let declaration = leaf.and_then(|name| {
            std::iter::successors(name.parent(), Node::parent)
                .find(|parent| matches!(parent.kind(), "parameter" | "let_declaration" | "match_arm" | "for_expression"))
        });
        let origin = if declaration.and_then(|decl| decl.child_by_field_name("type")).is_some() {
            TypeOrigin::Declared
        } else {
            TypeOrigin::Compiler
        };
        let mut shape = native_shape(Language::Rust, &ty);
        let mut native = ty.clone();
        if let Some(declaration) = declaration
            && let Some(pattern) = declaration.child_by_field_name("pattern")
            && let Some(leaf) = leaf
        {
            if let Some(written) = declaration.child_by_field_name("type") {
                if let Some(written) = project_pattern(src, pattern, written, leaf) {
                    shape = with_type_identity(shape, &written_shape(written, text, identities));
                    native = text[written.byte_range()].to_owned();
                }
            } else if let Some(value) = declaration.child_by_field_name("value")
                && let Some(value) = project_pattern(src, pattern, value, leaf)
                && let Some(evidence) = initializer(value, &types)
            {
                shape = with_type_identity(shape, &evidence);
                typed_initializers.insert((value.start_byte(), value.end_byte()));
                facts.types.push(TypeFact { site: site(value.start_byte(), value.end_byte()), target: ValueFactTarget::Expression,
                    info: TypeInfo::new(shape.clone(), TypeOrigin::Compiler, Some(ty)) });
            }
        }
        let info = TypeInfo::new(shape, origin, Some(native));
        facts.types.push(TypeFact { site: site(start, end), target: ValueFactTarget::Binding, info: info.clone() });
        types.insert(symbol, info);
    }
    for &(start, end) in initializers.shapes.keys().chain(initializers.calls.keys()) {
        if !typed_initializers.contains(&(start, end))
            && let Some(value) = root.descendant_for_byte_range(start, end)
            && let Some(shape) = initializer(value, &types)
        {
            facts.types.push(TypeFact { site: site(start, end), target: ValueFactTarget::Expression,
                info: TypeInfo::new(shape, TypeOrigin::Compiler, None) });
        }
    }
    for &(start, end, symbol) in uses {
        if let Some(info) = types.get(symbol) {
            let mut info = info.clone();
            info.origin = TypeOrigin::Compiler;
            facts.types.push(TypeFact { site: site(start, end), target: ValueFactTarget::Expression, info });
        }
    }
}

fn convert(index: &scip::types::Index, root: &Path, prefix: &str, local_packages: &HashSet<String>, packages: &[crate::project::CargoPackage], upstream_indexmaps: &HashSet<String>, facts: &mut SemanticFacts) {
    // Where each project symbol is defined, and what every symbol is.
    let rel_of = |doc: &scip::types::Document| {
        let rel = doc.relative_path.replace('\\', "/");
        if prefix.is_empty() { rel } else { format!("{prefix}/{rel}") }
    };
    let mut texts: HashMap<String, (String, Vec<usize>)> = HashMap::new();
    for doc in &index.documents {
        let rel = rel_of(doc);
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else { continue };
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        texts.insert(rel, (text, starts));
    }
    let mut definitions: HashMap<&str, ByteRange> = HashMap::new();
    for doc in &index.documents {
        let rel = rel_of(doc);
        let Some((_, starts)) = texts.get(&rel) else { continue };
        for o in &doc.occurrences {
            if o.symbol_roles & 1 == 1
                && !o.symbol.starts_with("local ")
                && let Some((s, e)) = range_bytes(starts, &o.range)
            {
                definitions.entry(o.symbol.as_str()).or_insert(ByteRange { file: rel.clone(), start: s as u32, end: e as u32 });
            }
        }
    }

    let mut typed_documents = HashMap::new();
    let mut default_types = HashMap::new();
    for doc in &index.documents {
        let rel = rel_of(doc);
        let Some((text, starts)) = texts.get(&rel) else { continue };
        let Some(tree) = crate::parse(text) else { continue };
        let identities = written_type_identities(doc, starts, tree.root_node(), text);
        default_types.insert(rel.clone(), default_type_identities(doc, starts, tree.root_node(), upstream_indexmaps));
        typed_documents.insert(rel, (tree, identities));
    }
    let mut cast_aliases = resolved_cast_aliases(index, &rel_of, &texts, &typed_documents);
    let cast_alias_at = |file: &str, ty: Node<'_>| {
        let ty = crate::syntax::unparen_type(ty);
        cast_aliases.get(file)?.get(&(ty.start_byte(), ty.end_byte())).cloned()
    };
    let return_types: HashMap<_, _> = definitions.iter().filter_map(|(&symbol, definition)| {
        if !matches!(kind_of(parse_symbol(symbol)?.descriptors), "method" | "function") { return None; }
        let (text, _) = texts.get(&definition.file)?;
        let (tree, identities) = typed_documents.get(&definition.file)?;
        let name = tree.root_node().descendant_for_byte_range(definition.start as usize, definition.end as usize)?;
        let function = name.parent().filter(|parent| matches!(parent.kind(), "function_item" | "function_signature_item"))?;
        let ty = function.child_by_field_name("return_type")?;
        let shape = written_shape(ty, text, identities);
        let info = TypeInfo::new(shape, TypeOrigin::Declared, Some(text[ty.byte_range()].to_owned()));
        let generics = type_parameters(function, text);
        let implementation = function.parent().and_then(|body| body.parent()).filter(|node| node.kind() == "impl_item");
        let owner = implementation.and_then(|node| node.child_by_field_name("type")).map(|ty| written_shape(ty, text, identities));
        let owner_generics = implementation.into_iter().flat_map(|node| type_parameters(node, text)).flatten().collect();
        let parameters = function.child_by_field_name("parameters").map(named_children).unwrap_or_default();
        let receiver = parameters.iter().find(|param| !param.is_extra()).is_some_and(|param| param.kind() == "self_parameter");
        let parameters = parameters.into_iter().filter(|param| !param.is_extra()).map(|param| {
            param.child_by_field_name("type").map(|ty| written_shape(ty, text, identities))
        }).collect();
        let cast_alias = cast_alias_at(&definition.file, ty);
        let default_collection = (function.kind() == "function_item"
            && !named_children(function).iter().any(|part| part.kind() == "function_modifiers"
                && nodex_lang_api::syntax::children(*part).iter().any(|token| token.kind() == "async")))
            .then(|| written_default_collection(ty, default_types.get(&definition.file)?)).flatten();
        Some((symbol, (definition, ReturnType { info, cast_alias, default_collection, generics, parameters, receiver, owner, owner_generics })))
    }).collect();
    for (definition, result) in return_types.values() {
        facts.types.push(TypeFact { site: (*definition).clone(), target: ValueFactTarget::Member, info: result.info.clone() });
    }
    let fields: HashMap<_, _> = definitions.iter().filter_map(|(&symbol, definition)| {
        let (text, _) = texts.get(&definition.file)?;
        let (tree, identities) = typed_documents.get(&definition.file)?;
        let name = tree.root_node().descendant_for_byte_range(definition.start as usize, definition.end as usize)?;
        let field = name.parent().filter(|node| node.kind() == "field_declaration")?;
        let ty = field.child_by_field_name("type")?;
        let owner = field.parent()?.parent()?.child_by_field_name("name")?;
        let owner = ByteRange { file: definition.file.clone(), start: owner.start_byte() as u32, end: owner.end_byte() as u32 };
        let info = TypeInfo::new(written_shape(ty, text, identities), TypeOrigin::Declared, Some(text[ty.byte_range()].to_owned()));
        let default_collection = default_types.get(&definition.file).and_then(|types| written_default_collection(ty, types));
        Some((symbol, (owner, info, cast_alias_at(&definition.file, ty), default_collection)))
    }).collect();

    for doc in &index.documents {
        let rel = rel_of(doc);
        let Some((text, starts)) = texts.get(&rel) else { continue };
        let Some((tree, identities)) = typed_documents.get(&rel) else { continue };
        let root_node = tree.root_node();
        let line_index = LineIndex::new(text);
        let site = |a: usize, b: usize| ByteRange { file: rel.clone(), start: a as u32, end: b as u32 };
        let first_nominal = facts.rust_nominal_types.len();
        let first_type = facts.types.len();
        let mut local_definitions = HashMap::new();
        let mut local_uses = Vec::new();
        let mut initializers = InitializerTypes::default();
        let mut arithmetic_outputs = HashMap::new();
        let mut operations = Operations::new();
        let mut non_bindings = HashSet::new();
        let mut field_uses = Vec::new();
        let mut alias_values = HashMap::new();
        let mut default_values = HashMap::new();
        let vec_macros: HashSet<_> = doc.occurrences.iter().filter_map(|occurrence| {
            let symbol = parse_symbol(&occurrence.symbol)?;
            (symbol.standard() && matches!(display(&symbol).as_str(), "alloc::macros::vec!" | "alloc::vec!" | "std::vec!"))
                .then(|| range_bytes(starts, &occurrence.range)).flatten()
        }).collect();
        let signatures: HashMap<&str, &str> = doc
            .symbols
            .iter()
            .filter_map(|s| s.signature_documentation.as_ref().map(|d| (s.symbol.as_str(), d.text.as_str())))
            .collect();
        let mut seen: HashSet<(usize, usize, String)> = HashSet::new();
        for o in &doc.occurrences {
            let Some((s, e)) = range_bytes(starts, &o.range) else { continue };
            // Local definitions: their types.
            if o.symbol.starts_with("local ") {
                if o.symbol_roles & 1 == 1 {
                    local_definitions.insert(o.symbol.as_str(), (s, e));
                } else {
                    local_uses.push((s, e, o.symbol.as_str()));
                }
                if o.symbol_roles & 1 == 1
                    && let Some(sig) = signatures.get(o.symbol.as_str())
                    && let Some((name, ty)) = local_signature(sig)
                {
                    facts.locals.push(LocalTypeFact { site: site(s, e), name, ty });
                }
                continue;
            }
            if o.symbol_roles & 1 == 1 {
                continue;
            }
            let Some(sym) = parse_symbol(&o.symbol) else { continue };
            let external = !local_packages.contains(sym.package);
            let Some(name) = root_node.descendant_for_byte_range(s, e) else { continue };
            if name.kind() == "identifier" && name.start_byte() == s && name.end_byte() == e {
                non_bindings.insert((s, e));
            }
            if sym.descriptors.ends_with('!') {
                if let Some(invocation) = macro_invocation(name) {
                    if vec_macros.contains(&(s, e))
                        && let Some(macro_name) = invocation.child_by_field_name("macro")
                        && let Some(tokens) = named_children(invocation).into_iter().find(|node| node.kind() == "token_tree")
                    {
                        initializers.shapes.insert((invocation.start_byte(), invocation.end_byte()),
                            vec_macro_shape(tokens, &text[macro_name.byte_range()], text, &vec_macros, 0));
                    }
                    let definition = definitions.get(o.symbol.as_str()).and_then(|definition| {
                        let (text, _) = texts.get(&definition.file)?;
                        Some(LineIndex::new(text).span(&definition.file, definition.start as usize, definition.end as usize))
                    });
                    let builtin = reading_operation(&sym, &display(&sym), external);
                    let name = builtin.map_or_else(String::new, |(_, name)| name);
                    record_operation(&mut operations, invocation, Resolved { family: Family::Macro, name, definition });
                }
                continue;
            }
            let kind = kind_of(sym.descriptors);
            let segs = segments(sym.descriptors);
            let target = SemanticTarget {
                decl: if external { None } else { definitions.get(o.symbol.as_str()).cloned() },
                display: display(&sym),
                container: (segs.len() >= 2).then(|| segs[segs.len() - 2].clone()),
                kind: kind.to_owned(),
                linq: false,
                external,
            };
            if sym.descriptors.ends_with("().")
                && let Some(expression) = name.parent().filter(|expr| match expr.kind() {
                    "binary_expression" | "compound_assignment_expr" => expr.child_by_field_name("operator") == Some(name),
                    "unary_expression" => expr.child(0) == Some(name),
                    "index_expression" => name.kind() == "[",
                    _ => false,
                })
            {
                if expression.kind() == "binary_expression" && name.start_byte() == s && name.end_byte() == e {
                    let range = (expression.start_byte(), expression.end_byte());
                    let output = arithmetic_result(&sym, name.kind());
                    arithmetic_outputs.entry(range).and_modify(|previous| {
                        if *previous != output { *previous = None; }
                    }).or_insert(output);
                }
                let boolean_assignment = if expression.kind() == "compound_assignment_expr"
                    && name.start_byte() == s && name.end_byte() == e
                    && sym.package == "core" && sym.standard()
                {
                    match (name.kind(), sym.descriptors) {
                        ("|=", "ops/bit/impl#[bool][`BitOrAssign<Self>`]bitor_assign().") => Some((
                            "Update a boolean with OR",
                            "Evaluate the right-hand expression even when the destination is already true. If evaluation completes, store true in the destination when either boolean is true. Otherwise store false.",
                        )),
                        ("&=", "ops/bit/impl#[bool][`BitAndAssign<Self>`]bitand_assign().") => Some((
                            "Update a boolean with AND",
                            "Evaluate the right-hand expression even when the destination is already false. If evaluation completes, store true in the destination when both booleans are true. Otherwise store false.",
                        )),
                        _ => None,
                    }
                } else {
                    None
                };
                if boolean_assignment.is_some() {
                    facts.expressions.push(ExpressionFact {
                        site: site(expression.start_byte(), expression.end_byte()),
                        kind: ExpressionFactKind::BooleanAssignment,
                        ty: None,
                    });
                }
                let (label, text) = boolean_assignment.map_or_else(
                    || ("Resolve the operator", format!("This operator calls {}.", target.display)),
                    |(label, text)| (label, text.into()),
                );
                facts.reading.push(ReadingFact {
                    anchor: line_index.span(&rel, expression.start_byte(), expression.end_byte()),
                    span: line_index.span(&rel, name.start_byte(), name.end_byte()),
                    basis: ValueBasis::TypeAnalysis,
                    expression_target: None,
                    content: ReadingContent::Detail {
                        label: label.into(),
                        text,
                        definition: None,
                        borrow: false,
                    },
                });
            }
            if let Some((family, operation)) = reading_operation(&sym, &target.display, external) {
                let at = if name.kind() == "?" && operation == "branch" && sym.descriptors.ends_with("][Try]branch().") {
                    name.parent().and_then(|parent| match parent.kind() {
                        "try_expression" => Some(parent),
                        // Known macros may reparse their operands. Retain evidence at the original token.
                        "token_tree" if name.start_byte() == s && name.end_byte() == e => Some(name),
                        _ => None,
                    })
                } else { enclosing_call(name) };
                if let Some(at) = at {
                    record_operation(&mut operations, at, Resolved { family, name: operation, definition: None });
                }
            }
            if let Some(field) = name.parent().filter(|parent| parent.kind() == "field_expression"
                && parent.child_by_field_name("field") == Some(name))
            {
                let entry = fields.get(o.symbol.as_str());
                record_unambiguous(&mut alias_values, (field.start_byte(), field.end_byte()),
                    entry.and_then(|(_, _, alias, _)| alias.as_ref()));
                record_unambiguous(&mut default_values, (field.start_byte(), field.end_byte()),
                    entry.and_then(|(_, _, _, proof)| *proof));
                if let Some((owner, info, _, _)) = entry {
                    field_uses.push((field, owner, info));
                }
            }
            let mut add_value = |node: Node, fact_target, mut info: ValueInfo| {
                info.basis = ValueBasis::TypeAnalysis;
                facts.values.push(ValueFact { site: site(node.start_byte(), node.end_byte()), target: fact_target, info });
            };
            if kind == "type"
                && let Some((implementation, false)) = outer_type_declaration(name)
                && implementation.kind() == "impl_item"
                && let Some(declaration) = &target.decl
                && let Some(body) = implementation.child_by_field_name("body")
            {
                for method in named_children(body).into_iter().filter(|child| child.kind() == "function_item") {
                    let Some(params) = method.child_by_field_name("parameters") else { continue };
                    let Some(receiver) = named_children(params).into_iter().find(|param| param.kind() == "self_parameter") else { continue };
                    let children = nodex_lang_api::syntax::children(receiver);
                    if let Some(binding) = children.iter().find(|child| child.kind() == "self") {
                        facts.rust_nominal_types.push(RustNominalType {
                            site: site(binding.start_byte(), binding.end_byte()),
                            declaration: declaration.clone(),
                            by_reference: children.iter().any(|child| child.kind() == "&"),
                        });
                    }
                }
            }
            if kind == "type"
                && let Some((decl, by_reference)) = outer_type_declaration(name)
                && let Some((binding, fact_target)) = named_value_site(decl)
            {
                if fact_target == ValueFactTarget::Member
                    && let Some(ty) = decl.child_by_field_name("type")
                {
                    facts.types.push(TypeFact { site: site(binding.start_byte(), binding.end_byte()), target: fact_target,
                        info: TypeInfo::new(written_shape(ty, text, identities), TypeOrigin::Declared, Some(text[ty.byte_range()].into())) });
                }
                if let Some(declaration) = &target.decl {
                    facts.rust_nominal_types.push(RustNominalType {
                        site: site(binding.start_byte(), binding.end_byte()),
                        declaration: declaration.clone(),
                        by_reference,
                    });
                }
                let known = (!by_reference).then(|| nodex_lang_api::value_shapes::written_type(Language::Rust, &target.display)
                    .or_else(|| {
                        let definition = definitions.get(o.symbol.as_str()).filter(|definition| definition.file == rel)?;
                        let name = root_node.descendant_for_byte_range(definition.start as usize, definition.end as usize)?;
                        local_nominal_type(name).then_some(ValueInfo {
                            kind: ValueKind::Value, basis: ValueBasis::TypeAnalysis, qualifier: None, passing: None,
                        })
                    })).flatten();
                if let Some(info) = known { add_value(binding, fact_target, info); }
            }
            if sym.standard()
                && let Some(owner) = target.display.strip_suffix("()").and_then(|path| path.rsplit_once("::")).map(|(owner, _)| owner)
                && let Some(call) = enclosing_call(name)
                // A method such as pop() has the same owner, but does not construct that collection.
                && let Some(method) = target.display.rsplit("::").next().filter(|method|
                    matches!(*method, "new()" | "with_capacity()" | "from()" | "from_iter()" | "default()"))
                && !call.child_by_field_name("function").is_some_and(|function| function.kind() == "field_expression")
            {
                let qualifier = if method == "default()" && owner == "core::default::Default" {
                    call.child_by_field_name("function")
                        .and_then(|function| function.child_by_field_name("path"))
                        .map(|ty| written_shape(ty, text, identities))
                } else { None };
                let shape = qualifier.unwrap_or_else(|| native_shape(Language::Rust, owner));
                if matches!(shape, TypeShape::Collection { .. } | TypeShape::Scalar { .. }) {
                    initializers.shapes.insert((call.start_byte(), call.end_byte()), shape);
                }
            }
            if let Some(owner) = target.display.strip_suffix("::new()")
                && let Some(info) = nodex_lang_api::value_shapes::written_type(Language::Rust, owner)
                && let Some(call) = enclosing_call(name)
            {
                add_value(call, ValueFactTarget::Expression, info);
                if let Some(decl) = call.parent().filter(|parent| parent.kind() == "let_declaration")
                    && decl.child_by_field_name("type").is_none()
                    && decl.child_by_field_name("value").is_some_and(|value| value.id() == call.id())
                    && let Some((binding, fact_target)) = named_value_site(decl)
                {
                    add_value(binding, fact_target, info);
                }
            }
            let (ref_kind, at) = match kind {
                "method" | "function" => match enclosing_call(name) {
                    Some(call) => (RefKind::Call, call),
                    None => continue,
                },
                "type" => match enclosing_struct_literal(name).or_else(|| enclosing_call(name)) {
                    Some(n) => (RefKind::Construct, n),
                    None => {
                        if let Some(declaration) = &target.decl {
                            let mut value = name;
                            if let Some(path) = value.parent().filter(|parent| parent.kind() == "scoped_identifier"
                                && parent.child_by_field_name("name").is_some_and(|last| last.id() == name.id())) {
                                value = path;
                            }
                            let sites = constructor_type_sites(value);
                            if sites.len() > 1 || value.parent().is_some_and(|parent| parent.kind() == "field_expression"
                                && parent.child_by_field_name("value").is_some_and(|base| base.id() == value.id())) {
                                facts.rust_nominal_types.extend(sites.into_iter().map(|(node, by_reference)| {
                                    RustNominalType { site: site(node.start_byte(), node.end_byte()), declaration: declaration.clone(), by_reference }
                                }));
                            }
                        }
                        continue;
                    }
                },
                _ => continue,
            };
            let key = (at.start_byte(), at.end_byte(), o.symbol.clone());
            if seen.insert(key) {
                if ref_kind == RefKind::Call {
                    let returns = return_types.get(o.symbol.as_str()).map(|(_, result)| result);
                    record_unambiguous(&mut alias_values, (at.start_byte(), at.end_byte()),
                        returns.and_then(|result| result.cast_alias.as_ref()));
                    record_unambiguous(&mut default_values, (at.start_byte(), at.end_byte()),
                        returns.and_then(|result| result.default_collection));
                }
                if ref_kind == RefKind::Call && let Some((_, result)) = return_types.get(o.symbol.as_str()) {
                    let range = (at.start_byte(), at.end_byte());
                    if result.generics.is_empty() && result.owner.is_none() {
                        initializers.shapes.insert(range, result.info.shape.clone());
                    } else {
                        initializers.calls.insert(range, result);
                    }
                }
                if ref_kind == RefKind::Construct && let Some(declaration) = &target.decl {
                    facts.rust_nominal_types.extend(constructor_type_sites(at).into_iter().map(|(node, by_reference)| {
                        RustNominalType { site: site(node.start_byte(), node.end_byte()), declaration: declaration.clone(), by_reference }
                    }));
                }
                facts.refs.push(SemanticRef { site: site(at.start_byte(), at.end_byte()), kind: ref_kind, target, emit_focus: None });
            }
        }
        rust_type_facts(facts, &rel, text, root_node, &local_definitions, &local_uses, &signatures, identities, &initializers);
        for ((start, end), shape) in arithmetic_outputs {
            let Some(shape) = shape else { continue };
            facts.types.push(TypeFact {
                site: site(start, end),
                target: ValueFactTarget::Expression,
                info: TypeInfo::new(shape, TypeOrigin::Compiler, None),
            });
        }
        local_uses.sort_unstable_by_key(|&(start, _, _)| start);
        propagate_nominal_locals(&mut facts.rust_nominal_types, first_nominal, &rel, root_node, &local_definitions, &local_uses);

        let mut effects: Vec<_> = operations.iter().filter_map(|(&range, operation)| {
            let operation = operation.as_ref()?;
            let kind = match (operation.family, operation.name.as_str()) {
                (Family::Thread | Family::Task, "spawn") => ExpressionFactKind::Spawn,
                (Family::Effect, "sleep") => ExpressionFactKind::Sleep,
                (Family::Effect, "send") => ExpressionFactKind::ChannelSend,
                (Family::Effect, "exit") => ExpressionFactKind::ProcessExit,
                _ => return None,
            };
            Some((range, kind))
        }).collect();
        effects.sort_unstable_by_key(|(range, _)| *range);
        facts.expressions.extend(effects.into_iter().map(|((start, end), kind)| ExpressionFact {
            site: site(start, end),
            kind,
            ty: None,
        }));
        let file = root.join(&rel);
        let edition = packages.iter().rev().find(|package| package.manifest.parent().is_some_and(|dir| file.starts_with(dir)))
            .and_then(|package| package.edition.as_deref());
        let mut cast_aliases = cast_aliases.remove(&rel).unwrap_or_default();
        cast_aliases.extend(alias_values.into_iter().filter_map(|(range, alias)| {
            Some((range, alias.flatten()?.clone()))
        }));
        let mut evidence = Evidence {
            operations,
            non_bindings,
            types: &facts.types[first_type..],
            type_identities: Some(identities),
            cast_aliases,
            default_types: default_types.get(&rel),
            default_collections: default_values.into_iter()
                .filter_map(|(range, proof)| Some((range, proof.flatten()?))).collect(),
            definitions: Some(&local_definitions),
            uses: &local_uses,
            precise_captures: match edition {
                Some("2015" | "2018") => Some(false),
                Some("2021" | "2024") => Some(true),
                _ => None,
            },
            ..Default::default()
        };
        let uses: HashMap<_, _> = local_uses.iter().map(|&(s, e, symbol)| ((s, e), symbol)).collect();
        let mut nominals = HashMap::new();
        for fact in &facts.rust_nominal_types[first_nominal..] {
            let identity = (&fact.declaration, fact.by_reference);
            nominals.entry((fact.site.start as usize, fact.site.end as usize)).and_modify(|previous| {
                if *previous != Some(identity) { *previous = None; }
            }).or_insert(Some(identity));
        }
        for (field, owner, info) in field_uses {
            let range = (field.start_byte(), field.end_byte());
            evidence.fields.insert(range, info.clone());
            let Some(base) = field.child_by_field_name("value").map(crate::syntax::unparen) else { continue };
            let Some(declaration) = uses.get(&(base.start_byte(), base.end_byte())).and_then(|symbol| local_definitions.get(symbol)) else { continue };
            if let Some(Some((actual, by_reference))) = nominals.get(declaration)
                && *actual == owner
            {
                evidence.direct_fields.insert(range, *by_reference);
            }
        }
        let src = Src { text, file: &rel, index: &line_index };
        let (reading, borrows, inferred) = crate::reading::collect(src, root_node, &evidence);
        facts.reading.extend(reading.into_iter().filter(|fact| fact.basis == ValueBasis::TypeAnalysis));
        facts.rust_borrows.extend(borrows);
        facts.types.extend(inferred);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scip_symbols_read_as_paths() {
        let d = |s: &str| display(&parse_symbol(s).unwrap());
        assert_eq!(d("rust-analyzer cargo alloc https://github.com/rust-lang/rust/library/alloc vec/Vec#push()."), "alloc::vec::Vec::push()");
        assert_eq!(
            d("rust-analyzer cargo nodex-rust-sample 0.1.0 services/order_service/impl#[`OrderService<R>`]new()."),
            "nodex_rust_sample::services::order_service::OrderService::new()"
        );
        assert_eq!(d("rust-analyzer cargo app 0.1.0 order/impl#[ShipError][Display]fmt()."), "app::order::ShipError::fmt()");
        assert_eq!(kind_of("vec/Vec#push()."), "method");
        assert_eq!(kind_of("util/sleep_ms()."), "function");
        assert_eq!(kind_of("domain/order/Order#"), "type");
        assert_eq!(local_signature("let mut total: i64"), Some(("total".into(), "i64".into())));
        assert_eq!(local_signature("customer: &str"), Some(("customer".into(), "&str".into())));
        assert_eq!(local_signature("self: &OrderService<R>"), None);
    }

    #[test]
    fn recognizes_tokio_select_by_macro_identity() {
        let symbol = parse_symbol("rust-analyzer cargo tokio 1.47.1 macros/select/select!").unwrap();
        assert_eq!(reading_operation(&symbol, &display(&symbol), true), Some((Family::Macro, "select".into())));
        assert!(reading_operation(&symbol, &display(&symbol), false).is_none());
    }

    #[test]
    fn recognizes_only_the_standard_iterator_callback_overrides() {
        let standard = "rust-analyzer cargo core https://github.com/rust-lang/rust/library/core";
        let override_ = "slice/iter/impl#[`Iter<'a, T>`][Iterator]find_map().";
        let resolved = |symbol: &str| {
            let parsed = parse_symbol(symbol).unwrap();
            reading_operation(&parsed, &display(&parsed), true)
        };
        assert_eq!(resolved(&format!("{standard} {override_}")), Some((Family::Iter, "find_map".into())));
        assert_eq!(resolved(&format!("rust-analyzer cargo local 0.1.0 {override_}")), None);
        assert_eq!(resolved(&format!("{standard} {}", override_.replace("[Iterator]", "[Other]"))), None);
        assert_eq!(resolved(&format!("{standard} {}", override_.replace("find_map", "map"))), None);
    }
}
