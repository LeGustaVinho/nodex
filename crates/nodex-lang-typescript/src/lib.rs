//! # Nodex TypeScript frontend
//!
//! Lowers TypeScript and JavaScript (`.ts .tsx .mts .cts .js .jsx .mjs .cjs`) to the Nodex IR
//! using tree-sitter. It needs no `npm install` and no build: structure and flow graphs come
//! from syntax alone. JavaScript is parsed with the TSX grammar (a superset of JavaScript with
//! JSX) and badged `javascript`.

mod expr;
mod flow;
mod frameworks;
mod debug;
mod instrument;
mod knowledge;
mod prepare;
mod project;
pub mod semantic;
mod structure;
mod syntax;
mod vue;

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
        &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue"]
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

    fn debugger(&self, _root: &Path) -> Option<Box<dyn nodex_lang_api::DebugLauncher>> {
        Some(Box::new(debug::NodeDebugger))
    }

    fn instrumenter(&self, _root: &Path) -> Option<Box<dyn TraceInstrumenter>> {
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

/// What the parser reads for a file: the text itself, or the script of a Vue component.
fn parser_input(rel: &str, text: &str) -> (String, bool, bool) {
    if extension(rel) == "vue" {
        let s = vue::script(text);
        (s.text, !s.typescript, s.setup)
    } else {
        (text.to_owned(), uses_tsx(rel), false)
    }
}

/// Parses a file with the right grammar through a prepared copy (see [`prepare`]).
fn parse(rel: &str, text: &str) -> Option<prepare::Prepared> {
    let (input, tsx, _) = parser_input(rel, text);
    parse_as(tsx, &input)
}

fn parse_as(tsx: bool, text: &str) -> Option<prepare::Prepared> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar(tsx)).ok()?;
    prepare::parse(&mut parser, text)
}

/// The rewrites a file needed to parse (see [`prepare`]); a diagnostic aid.
pub fn grammar_bridges(rel: &str, text: &str) -> Vec<&'static str> {
    parse(rel, text).map(|p| p.rules).unwrap_or_default()
}

/// Where the grammar fails: `(line, first line of the failing code)`. A diagnostic aid for
/// corpus testing.
pub fn remaining_parse_errors(rel: &str, text: &str) -> Vec<(usize, String)> {
    let Some(prepared) = parse(rel, text) else { return Vec::new() };
    let tree = prepared.tree;
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
        borrow_models: Default::default(),
        source_revision: None,
        file: rel_path.to_owned(),
        language: Language::Typescript,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
        knowledge: Default::default(),
    };
    if rel_path.ends_with(".min.js") || rel_path.ends_with(".min.mjs") {
        ir.diagnostics.push(Diagnostic { severity: Severity::Info, message: "Minified file: not shown.".into(), span: None });
        return ir;
    }
    // Vue components: only their scripts are code; the rest of the file is blanked for everything
    // that reads text (labels, spans and line numbers still match the real file).
    let (input, tsx, vue_setup) = parser_input(rel_path, text);
    let fw = frameworks::detect(rel_path, text);
    let original = text;
    let text = input.as_str();
    let Some(prepare::Prepared { tree, .. }) = parse_as(tsx, text) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "tree-sitter TypeScript grammar is incompatible with the runtime".into(),
            span: None,
        });
        return ir;
    };
    let index = LineIndex::new(original);
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
    let handlers = frameworks::template_handlers(original);
    let mut collector = structure::Collector::new(src, is_javascript(rel_path), is_dts, fw, vue_setup, handlers);
    collector.program(root);
    let structure::Collected { mut symbols, imports, mut type_refs, bodies, function_ids } = collector.finish();

    let mut continuations = std::collections::HashMap::new();
    for job in &bodies {
        let lowered = flow::lower_body(src, job, &function_ids, fw);
        type_refs.extend(lowered.type_refs);
        if let Some(sym) = symbols.iter_mut().find(|s| s.id == job.symbol) {
            sym.has_flow = true;
        }
        if !lowered.continuations.is_empty() {
            continuations.insert(job.symbol.clone(), lowered.continuations);
        }
        ir.flows.push(lowered.graph);
    }

    ir.knowledge = knowledge::extract(src, root, &symbols, &function_ids, original);
    let from_scope = module_path(rel_path);
    ir.imports = imports.into_iter().map(|target| ImportRef { from_scope: from_scope.clone(), target }).collect();
    ir.symbols = symbols;
    ir.type_refs = type_refs;
    nodex_lang_api::syntax_sites::annotate(&mut ir, root, src, is_javascript(rel_path));
    nodex_lang_api::value_shapes::annotate(&mut ir, root, src);
    nodex_lang_api::blueprint_types::annotate(&mut ir, root, src, &continuations);
    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_types_parse_through_a_same_length_copy() {
        let text = "let a: import('./x.js').T[] = [];\ntype F = typeof import(\"node:fs\");\nconst m = await import('./m.js');\n";
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
