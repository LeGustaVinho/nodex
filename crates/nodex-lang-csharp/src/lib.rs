//! # Nodex C# frontend
//!
//! Lowers C# (plain .NET and Unity) to the Nodex IR using tree-sitter. It needs no build and no
//! restore: structure and flow graphs come from syntax alone.
//!
//! The tree-sitter grammar predates some syntax; [`prepare`] adapts a copy of the text for the
//! parser without moving any byte, while labels and spans keep reading the original. The Roslyn sidecar ([`roslyn`])
//! adds exact semantics when the .NET SDK is available.

mod expr;
mod flow;
mod debug;
mod instrument;
mod knowledge;
mod prepare;
mod project;
pub mod roslyn;
pub mod sidecar;
mod structure;
mod syntax;
mod unity;
pub mod unity_editor;

use std::path::Path;

use nodex_ir::{Diagnostic, FileIr, Language, Severity};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SemanticProvider, SourceFile, TraceInstrumenter, kit::LineIndex};

/// The C# language frontend.
#[derive(Debug, Default, Clone, Copy)]
pub struct CSharpFrontend;

impl LanguageFrontend for CSharpFrontend {
    fn id(&self) -> Language {
        Language::Csharp
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }

    fn detect(&self, root: &Path) -> Option<ProjectDescriptor> {
        project::detect(root)
    }

    fn lower_file(&self, file: &SourceFile) -> FileIr {
        lower_source(&file.rel_path, &file.text)
    }

    fn semantic(&self, root: &Path) -> Option<Result<Box<dyn SemanticProvider>, String>> {
        Some(roslyn::RoslynSidecar::start(root).map(|s| Box::new(s) as Box<dyn SemanticProvider>))
    }

    /// Unity projects are debugged in the Editor (unity-dap attaches to it); .NET programs are
    /// built and launched under netcoredbg.
    fn debugger(&self, root: &Path) -> Option<Box<dyn nodex_lang_api::DebugLauncher>> {
        if unity_editor::is_unity(root) {
            return Some(Box::new(unity_editor::UnityDebugger));
        }
        Some(Box::new(debug::DotnetDebugger))
    }

    /// Unity projects trace Play Mode in the open Editor (the Nodex package compiles the probes
    /// in); .NET programs are built, woven and run by Nodex.
    fn instrumenter(&self, root: &Path) -> Option<Box<dyn TraceInstrumenter>> {
        if unity_editor::is_unity(root) {
            return Some(Box::new(unity_editor::UnityInstrumenter));
        }
        Some(Box::new(instrument::DotnetInstrumenter))
    }
}

/// Where the grammar still fails after preparation: `(line, first line of the error)`.
/// A diagnostic aid for corpus testing.
pub fn remaining_parse_errors(text: &str) -> Vec<(usize, String)> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).is_err() {
        return Vec::new();
    }
    let prepared = prepare::prepare(&mut parser, text);
    let parse_text = prepared.text.as_deref().unwrap_or(text);
    let Some(tree) = parser.parse(parse_text, None) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        // Report the innermost failures: an ERROR node may wrap a whole namespace.
        let mut c = n.walk();
        let failing_children: Vec<_> = n.children(&mut c).filter(|k| k.is_error() || k.is_missing() || k.has_error()).collect();
        if (n.is_error() || n.is_missing()) && failing_children.is_empty() {
            let first = text.get(n.byte_range()).and_then(|t| t.lines().find(|l| !l.trim().is_empty())).unwrap_or("").trim();
            out.push((n.start_position().row + 1, format!("{}{}", if n.is_missing() { "missing " } else { "" }, first.chars().take(80).collect::<String>())));
            continue;
        }
        if n.is_error() {
            stack.extend(failing_children);
            // Leaf tokens directly inside an ERROR are what the grammar could not place.
            let mut c = n.walk();
            if let Some(first) = n.children(&mut c).find(|k| !k.is_named() || k.kind() == "identifier") {
                out.push((first.start_position().row + 1, format!("near `{}`", text.get(first.byte_range()).unwrap_or(""))));
            }
            continue;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    out.sort();
    out
}

/// Lowers C# source text. Never fails: parse problems become diagnostics and `Opaque` nodes.
pub fn lower_source(rel_path: &str, text: &str) -> FileIr {
    let mut parser = tree_sitter::Parser::new();
    let mut ir = FileIr {
        borrow_models: Default::default(),
        source_revision: None,
        file: rel_path.to_owned(),
        language: Language::Csharp,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
        knowledge: Default::default(),
    };
    if parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .is_err()
    {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "tree-sitter C# grammar is incompatible with the runtime".into(),
            span: None,
        });
        return ir;
    }
    // Parse a prepared copy (same length); read code and spans from the original.
    let mut prepared = prepare::prepare(&mut parser, text);
    let parse_text: &str = prepared.text.as_deref().unwrap_or(text);
    let Some(tree) = prepared.tree.take().or_else(|| parser.parse(parse_text, None)) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "parser returned no tree".into(),
            span: None,
        });
        return ir;
    };

    let index = LineIndex::new(text);
    let root = tree.root_node();
    for fix in &prepared.merged_directives {
        let how = if fix.kept == "merged" {
            "its variants are shown merged".to_owned()
        } else {
            format!("only the `{}` variant is shown", fix.kept)
        };
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            message: format!(
                "Conditional compilation at lines {}–{} splits a declaration or statement; {how}.",
                fix.first, fix.last
            ),
            span: None,
        });
    }
    if root.has_error() {
        let first_error = syntax::first_error(root);
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "The file has syntax errors; the affected code appears as Opaque nodes.".into(),
            span: first_error.map(|n| index.span(rel_path, n.start_byte(), n.end_byte())),
        });
    }

    let src = syntax::Src {
        text,
        file: rel_path,
        index: &index,
    };
    let mut collector = structure::Collector::new(src, prepared.extension_blocks);
    collector.compilation_unit(root);
    let structure::Collected {
        mut symbols,
        imports,
        mut type_refs,
        bodies,
    } = collector.finish();

    // Callbacks with a Blueprint of their own, by start byte (the lambda's `Expr::symbol`).
    let callbacks: std::collections::HashMap<usize, nodex_ir::SymbolId> = bodies
        .iter()
        .filter(|j| j.flavors.iter().any(|f| f == "callback"))
        .map(|j| (j.decl.start_byte(), j.symbol.clone()))
        .collect();
    for job in &bodies {
        let lowered = flow::lower_body(src, job, &callbacks);
        type_refs.extend(lowered.type_refs);
        if let Some(sym) = symbols.iter_mut().find(|s| s.id == job.symbol) {
            sym.has_flow = true;
            for f in lowered.symbol_flavors {
                if !sym.flavors.contains(&f) {
                    sym.flavors.push(f);
                }
            }
        }
        ir.flows.push(lowered.graph);
    }

    ir.knowledge = knowledge::extract(src, root, &symbols, &callbacks);
    ir.symbols = symbols;
    ir.imports = imports;
    ir.type_refs = type_refs;
    nodex_lang_api::syntax_sites::annotate(&mut ir, root, src, false);
    nodex_lang_api::value_shapes::annotate(&mut ir, root, src);
    nodex_lang_api::blueprint_types::annotate(&mut ir, root, src, &Default::default());
    ir
}
