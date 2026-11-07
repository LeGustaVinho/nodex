//! # Nodex TypeScript frontend
//!
//! Lowers TypeScript and JavaScript (`.ts .tsx .mts .cts .js .jsx .mjs .cjs`) to the Nodex IR
//! using tree-sitter. It needs no `npm install` and no build: structure and flow graphs come
//! from syntax alone. JavaScript is parsed with the TSX grammar (a superset of JavaScript with
//! JSX) and badged `javascript`.

mod expr;
mod flow;
mod instrument;
mod project;
pub mod semantic;
mod structure;
mod syntax;

use std::path::Path;

use nodex_ir::{Diagnostic, FileIr, ImportRef, Language, Severity};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SemanticProvider, SourceFile, TraceInstrumenter, kit::LineIndex};

pub use structure::{import_target, module_path};

/// The TypeScript/JavaScript language frontend.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeScriptFrontend;

impl LanguageFrontend for TypeScriptFrontend {
    fn id(&self) -> Language {
        Language::Typescript
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"]
    }

    fn detect(&self, root: &Path) -> Option<ProjectDescriptor> {
        project::detect(root)
    }

    fn lower_file(&self, file: &SourceFile) -> FileIr {
        lower_source(&file.rel_path, &file.text)
    }

    fn semantic(&self, root: &Path) -> Option<Result<Box<dyn SemanticProvider>, String>> {
        Some(semantic::TsSidecar::start(root).map(|s| Box::new(s) as Box<dyn SemanticProvider>))
    }

    fn instrumenter(&self) -> Option<Box<dyn TraceInstrumenter>> {
        Some(Box::new(instrument::NodeInstrumenter))
    }
}

/// The grammar for a file: TSX for JSX-capable files and all JavaScript, TypeScript otherwise
/// (where `<T>value` is a type assertion, not an element).
pub fn grammar(tsx: bool) -> tree_sitter::Language {
    if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }
}

fn extension(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel).rsplit_once('.').map(|(_, e)| e).unwrap_or("")
}

fn uses_tsx(rel: &str) -> bool {
    !matches!(extension(rel), "ts" | "mts" | "cts")
}

fn is_javascript(rel: &str) -> bool {
    matches!(extension(rel), "js" | "jsx" | "mjs" | "cjs")
}

/// A copy of `text` the parser accepts, with the same byte length (see ADR-011): a UTF-8 byte
/// order mark becomes three spaces. `None` when nothing needs changing.
fn prepare(text: &str) -> Option<String> {
    text.strip_prefix('\u{feff}').map(|rest| format!("   {rest}"))
}

/// Import types (`x: import('./a').T`, `typeof import('node:fs')`), which the grammar lacks:
/// the `import('…')` part becomes an identifier of the same length (`_____.T`).
fn rewrite_import_types(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    let mut from = 0;
    while let Some(pos) = text[from..].find("import(").map(|p| p + from) {
        from = pos + 7;
        let before = text[..pos].trim_end();
        let type_position = before.ends_with("typeof") || before.ends_with("readonly") || before.ends_with("keyof") || before.ends_with([':', '<', '|', '&']);
        if !type_position || (pos > 0 && (bytes[pos - 1].is_ascii_alphanumeric() || bytes[pos - 1] == b'_')) {
            continue;
        }
        let Some(close) = text[pos..].find(')').map(|c| c + pos) else { continue };
        let inner = text[pos + 7..close].trim();
        if !(inner.starts_with(['"', '\'']) && inner.ends_with(['"', '\''])) || text[pos..close].contains('\n') {
            continue;
        }
        for b in &mut out[pos..=close] {
            *b = b'_';
        }
        changed = true;
    }
    changed.then(|| String::from_utf8(out).ok()).flatten()
}

fn error_count(tree: &tree_sitter::Tree) -> usize {
    let mut count = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            count += 1;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    count
}

/// Parses a file with the right grammar; a prepared copy is used only if it parses better.
fn parse(rel: &str, text: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar(uses_tsx(rel))).ok()?;
    let prepared = prepare(text);
    let base = prepared.as_deref().unwrap_or(text);
    let tree = parser.parse(base, None)?;
    if !tree.root_node().has_error() || !base.contains("import(") {
        return Some(tree);
    }
    let Some(candidate) = rewrite_import_types(base) else { return Some(tree) };
    match parser.parse(&candidate, None) {
        Some(better) if error_count(&better) < error_count(&tree) => Some(better),
        _ => Some(tree),
    }
}

/// Where the grammar fails: `(line, first line of the failing code)`. A diagnostic aid for
/// corpus testing.
pub fn remaining_parse_errors(rel: &str, text: &str) -> Vec<(usize, String)> {
    let Some(tree) = parse(rel, text) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        let failing: Vec<_> = n.children(&mut c).filter(|k| k.is_error() || k.is_missing() || k.has_error()).collect();
        if (n.is_error() || n.is_missing()) && failing.is_empty() {
            let first = text.get(n.byte_range()).and_then(|t| t.lines().find(|l| !l.trim().is_empty())).unwrap_or("").trim();
            out.push((n.start_position().row + 1, format!("{}{}", if n.is_missing() { "missing " } else { "" }, first.chars().take(80).collect::<String>())));
            continue;
        }
        stack.extend(failing);
    }
    out.sort();
    out
}

/// Lowers TypeScript/JavaScript source. Never fails: parse problems become diagnostics and
/// `Opaque` nodes.
pub fn lower_source(rel_path: &str, text: &str) -> FileIr {
    let mut ir = FileIr {
        file: rel_path.to_owned(),
        language: Language::Typescript,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
    };
    if rel_path.ends_with(".min.js") || rel_path.ends_with(".min.mjs") {
        ir.diagnostics.push(Diagnostic { severity: Severity::Info, message: "Minified file: not shown.".into(), span: None });
        return ir;
    }
    let Some(tree) = parse(rel_path, text) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "tree-sitter TypeScript grammar is incompatible with the runtime".into(),
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
    let is_dts = rel_path.ends_with(".d.ts") || rel_path.ends_with(".d.mts") || rel_path.ends_with(".d.cts");
    let mut collector = structure::Collector::new(src, is_javascript(rel_path), is_dts);
    collector.program(root);
    let structure::Collected { mut symbols, imports, mut type_refs, bodies, function_ids } = collector.finish();

    for job in &bodies {
        let lowered = flow::lower_body(src, job, &function_ids);
        type_refs.extend(lowered.type_refs);
        if let Some(sym) = symbols.iter_mut().find(|s| s.id == job.symbol) {
            sym.has_flow = true;
        }
        ir.flows.push(lowered.graph);
    }

    let from_scope = module_path(rel_path);
    ir.imports = imports.into_iter().map(|target| ImportRef { from_scope: from_scope.clone(), target }).collect();
    ir.symbols = symbols;
    ir.type_refs = type_refs;
    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_types_parse_through_a_same_length_copy() {
        let text = "let a: import('./x.js').T[] = [];\ntype F = typeof import(\"node:fs\");\nconst m = await import('./m.js');\n";
        let rewritten = rewrite_import_types(text).expect("rewritten");
        assert_eq!(rewritten.len(), text.len());
        assert!(rewritten.contains("let a: ________________.T[]"));
        assert!(rewritten.contains(&format!("typeof {};", "_".repeat("import(\"node:fs\")".len()))));
        assert!(rewritten.contains("await import('./m.js')"), "dynamic imports stay");
        let ir = lower_source("a.ts", text);
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
    }

    #[test]
    fn byte_order_mark_does_not_break_parsing() {
        let ir = lower_source("bom.ts", "\u{feff}export function f() { return 1; }\n");
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        assert_eq!(ir.flows.len(), 1);
    }
}
