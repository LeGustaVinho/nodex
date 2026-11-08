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

use nodex_ir::semantic::{ByteRange, LocalTypeFact, RefKind, SemanticFacts, SemanticRef, SemanticTarget};
use nodex_lang_api::SemanticProvider;
use protobuf::Message;
use tree_sitter::Node;

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
            refs: Vec::new(),
            locals: Vec::new(),
            messages: Vec::new(),
        };
        for (n, crate_root) in self.crate_roots.iter().enumerate() {
            let index = self.index(crate_root, n)?;
            let prefix = crate_root.strip_prefix(&self.root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            convert(&index, &self.root, &prefix, &self.local_packages, &mut facts);
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
    descriptors: &'s str,
}

fn parse_symbol(symbol: &str) -> Option<Sym<'_>> {
    if symbol.starts_with("local ") {
        return None;
    }
    let mut parts = symbol.splitn(5, ' ');
    let (_scheme, _manager, package, _version, descriptors) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    Some(Sym { package, descriptors })
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

fn convert(index: &scip::types::Index, root: &Path, prefix: &str, local_packages: &HashSet<String>, facts: &mut SemanticFacts) {
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

    for doc in &index.documents {
        let rel = rel_of(doc);
        let Some((text, starts)) = texts.get(&rel) else { continue };
        let Some(tree) = crate::parse(text) else { continue };
        let root_node = tree.root_node();
        let signatures: HashMap<&str, &str> = doc
            .symbols
            .iter()
            .filter_map(|s| s.signature_documentation.as_ref().map(|d| (s.symbol.as_str(), d.text.as_str())))
            .collect();
        let mut seen: HashSet<(usize, usize, String)> = HashSet::new();
        for o in &doc.occurrences {
            let Some((s, e)) = range_bytes(starts, &o.range) else { continue };
            let site = |a: usize, b: usize| ByteRange { file: rel.clone(), start: a as u32, end: b as u32 };
            // Local definitions: their types.
            if o.symbol.starts_with("local ") {
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
            if sym.descriptors.ends_with('!') {
                continue;
            }
            let Some(name) = root_node.descendant_for_byte_range(s, e) else { continue };
            let kind = kind_of(sym.descriptors);
            let external = !local_packages.contains(sym.package);
            let segs = segments(sym.descriptors);
            let target = SemanticTarget {
                decl: if external { None } else { definitions.get(o.symbol.as_str()).cloned() },
                display: display(&sym),
                container: (segs.len() >= 2).then(|| segs[segs.len() - 2].clone()),
                kind: kind.to_owned(),
                linq: false,
                external,
            };
            let (ref_kind, at) = match kind {
                "method" | "function" => match enclosing_call(name) {
                    Some(call) => (RefKind::Call, call),
                    None => continue,
                },
                "type" => match enclosing_struct_literal(name).or_else(|| enclosing_call(name)) {
                    Some(n) => (RefKind::Construct, n),
                    None => continue,
                },
                _ => continue,
            };
            let key = (at.start_byte(), at.end_byte(), o.symbol.clone());
            if seen.insert(key) {
                facts.refs.push(SemanticRef { site: site(at.start_byte(), at.end_byte()), kind: ref_kind, target });
            }
        }
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
}
