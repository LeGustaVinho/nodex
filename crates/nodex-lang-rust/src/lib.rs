//! # Nodex Rust frontend
//!
//! Lowers Rust (`.rs`) to the Nodex IR using tree-sitter: modules, types, traits and `impl`
//! blocks become structure symbols, function bodies become Blueprints with the shared
//! vocabulary. Data wires carry Rust ownership (move, copy, borrow). Semantics come from
//! rust-analyzer ([`semantic`]); live traces from an instrumented copy of the crate
//! ([`instrument`]).

mod expr;
mod flow;
mod debug;
pub mod instrument;
mod knowledge;
pub mod project;
pub mod semantic;
mod structure;
mod syntax;

use std::collections::HashSet;
use std::path::Path;

use nodex_ir::{Diagnostic, FileIr, ImportRef, Language, Severity};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SemanticProvider, SourceFile, TraceInstrumenter, kit::LineIndex};

pub use structure::{module_of, module_path};

/// The Rust language frontend.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustFrontend;

impl LanguageFrontend for RustFrontend {
    fn id(&self) -> Language {
        Language::Rust
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn detect(&self, root: &Path) -> Option<ProjectDescriptor> {
        project::detect(root)
    }

    fn lower_file(&self, file: &SourceFile) -> FileIr {
        lower_source(&file.rel_path, &file.text)
    }

    fn semantic(&self, root: &Path) -> Option<Result<Box<dyn SemanticProvider>, String>> {
        Some(semantic::RustAnalyzer::start(root).map(|s| Box::new(s) as Box<dyn SemanticProvider>))
    }

    fn debugger(&self) -> Option<Box<dyn nodex_lang_api::DebugLauncher>> {
        Some(Box::new(debug::RustDebugger))
    }

    fn instrumenter(&self) -> Option<Box<dyn TraceInstrumenter>> {
        Some(Box::new(instrument::RustInstrumenter))
    }
}

/// The tree-sitter Rust grammar.
pub fn grammar() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// Parses Rust text. When the grammar fails, same-length rewrites bridge syntax it predates
/// (ADR-011); a rewrite is kept only if it reduces the errors. Spans and labels keep reading
/// the original text.
pub fn parse(text: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar()).ok()?;
    let tree = parser.parse(text, None)?;
    if !tree.root_node().has_error() {
        return Some(tree);
    }
    let mut best = (error_count(&tree), tree);
    let mut current = text.to_owned();
    for rule in [blank_safe_keyword as fn(&str) -> Option<String>, rename_try_macro] {
        let Some(next) = rule(&current) else { continue };
        debug_assert_eq!(next.len(), current.len());
        let Some(t) = parser.parse(&next, None) else { continue };
        let n = error_count(&t);
        if n < best.0 {
            best = (n, t);
            current = next;
        }
    }
    Some(best.1)
}

fn error_count(tree: &tree_sitter::Tree) -> usize {
    let mut count = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            count += 1;
            continue;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    count
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Byte offsets of `word` as a whole word.
fn word_at(text: &str, word: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    text.match_indices(word)
        .map(|(i, _)| i)
        .filter(|&i| (i == 0 || !is_ident(bytes[i - 1])) && bytes.get(i + word.len()).is_none_or(|&b| !is_ident(b)))
        .collect()
}

/// Rust 2024 `unsafe extern { safe fn f(); }`: the grammar does not know `safe`.
fn blank_safe_keyword(text: &str) -> Option<String> {
    let hits: Vec<usize> = word_at(text, "safe").into_iter().filter(|&i| text[i + 4..].trim_start().starts_with("fn") || text[i + 4..].trim_start().starts_with("static")).collect();
    if hits.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for i in hits {
        out.replace_range(i..i + 4, "    ");
    }
    Some(out)
}

/// `try!(x)` (Rust 2015): `try` is a keyword since 2018, so the grammar rejects the macro.
fn rename_try_macro(text: &str) -> Option<String> {
    let hits: Vec<usize> = word_at(text, "try").into_iter().filter(|&i| text[i + 3..].starts_with('!')).collect();
    if hits.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for i in hits {
        out.replace_range(i..i + 3, "Try");
    }
    Some(out)
}

/// Where the grammar fails: `(line, first line of the failing code)`. A diagnostic aid for
/// corpus testing.
pub fn remaining_parse_errors(text: &str) -> Vec<(usize, String)> {
    let text = without_bom(text);
    let Some(tree) = parse(&text) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        let failing: Vec<_> = n.children(&mut c).filter(|k| k.is_error() || k.is_missing() || k.has_error()).collect();
        if (n.is_error() || n.is_missing()) && failing.is_empty() {
            let first = text.get(n.byte_range()).and_then(|t| t.lines().find(|l| !l.trim().is_empty())).unwrap_or("").trim();
            out.push((n.start_position().row + 1, first.chars().take(80).collect::<String>()));
            continue;
        }
        stack.extend(failing);
    }
    out.sort();
    out
}

/// The text with a leading byte order mark replaced by spaces of the same byte length (spans
/// keep matching the file).
fn without_bom(text: &str) -> String {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => format!("   {rest}"),
        None => text.to_owned(),
    }
}

/// Lowers Rust source. Never fails: parse problems become diagnostics and `Opaque` nodes.
pub fn lower_source(rel_path: &str, text: &str) -> FileIr {
    let mut ir = FileIr {
        file: rel_path.to_owned(),
        language: Language::Rust,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
        knowledge: Default::default(),
    };
    let prepared = without_bom(text);
    let text = prepared.as_str();
    let Some(tree) = parse(text) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "tree-sitter Rust grammar is incompatible with the runtime".into(),
            span: None,
        });
        return ir;
    };
    let index = LineIndex::new(text);
    let root = tree.root_node();
    if root.has_error() {
        let first_error = syntax::first_error(root);
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "The file has syntax errors; the affected code appears as Opaque nodes.".into(),
            span: first_error.map(|n| index.span(rel_path, n.start_byte(), n.end_byte())),
        });
    }
    let src = syntax::Src { text, file: rel_path, index: &index };
    let mut collector = structure::Collector::new(src);
    collector.source_file(root);
    let structure::Collected { mut symbols, imports, mut type_refs, bodies, function_ids, module } = collector.finish();

    let copy_types: HashSet<String> = symbols
        .iter()
        .filter(|s| s.flavors.iter().any(|f| f == "derive:Copy"))
        .map(|s| s.name.clone())
        .collect();
    // Types of this file with `impl Drop`: their locals run `drop` when their scope ends.
    let drop_types: std::collections::HashMap<String, nodex_ir::SymbolId> = symbols
        .iter()
        .filter(|s| s.name == "drop" && s.flavors.iter().any(|f| f == "from-trait:Drop"))
        .filter_map(|s| {
            let owner = s.parent.as_ref()?;
            let ty = symbols.iter().find(|t| &t.id == owner)?;
            Some((ty.name.clone(), s.id.clone()))
        })
        .collect();
    for job in &bodies {
        let lowered = flow::lower_body(src, job, &function_ids, &copy_types, &drop_types);
        type_refs.extend(lowered.type_refs);
        if let Some(sym) = symbols.iter_mut().find(|s| s.id == job.symbol) {
            sym.has_flow = true;
        }
        ir.flows.push(lowered.graph);
    }
    ir.knowledge = knowledge::extract(src, root, &symbols);
    ir.imports = imports.into_iter().map(|target| ImportRef { from_scope: module.clone(), target }).collect();
    ir.symbols = symbols;
    ir.type_refs = type_refs;
    ir
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_ir::{Concept, SymbolKind};

    #[test]
    fn lowers_a_small_crate_file() {
        let text = r#"
//! Orders.
use std::collections::HashMap;

/// An order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order { pub id: u32, pub amount: i64 }

pub trait Priced { fn price(&self) -> i64; }

impl Priced for Order {
    fn price(&self) -> i64 { self.amount }
}

pub fn sum_of_even_squares(numbers: &[i32]) -> i32 {
    let mut total = 0;
    for n in numbers {
        if n % 2 == 0 {
            total += n * n;
        }
    }
    total
}
"#;
        let ir = lower_source("src/orders.rs", text);
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        let order = ir.symbols.iter().find(|s| s.id == "crate::orders::Order").expect("struct");
        assert_eq!(order.kind, SymbolKind::Struct);
        assert!(order.flavors.iter().any(|f| f == "derive:Copy"));
        assert_eq!(order.doc.as_deref(), Some("An order."));
        assert!(ir.symbols.iter().any(|s| s.id == "crate::orders::Order::price" && s.kind == SymbolKind::Method));
        assert!(ir.symbols.iter().any(|s| s.id == "crate::orders::Order::amount"));
        assert!(ir.imports.iter().any(|i| i.target == "pkg:std"));
        let flow = ir.flows.iter().find(|f| f.symbol == "crate::orders::sum_of_even_squares").expect("flow");
        let concepts: Vec<Concept> = flow.nodes.iter().map(|n| n.concept).collect();
        assert_eq!(concepts, [Concept::Entry, Concept::Var, Concept::LoopForEach, Concept::Branch, Concept::Assign, Concept::Return]);
    }

    #[test]
    fn byte_order_mark_does_not_break_parsing() {
        let ir = lower_source("src/lib.rs", "\u{feff}pub fn f() -> i32 { 1 }\n");
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        assert_eq!(ir.flows.len(), 1);
    }
}
