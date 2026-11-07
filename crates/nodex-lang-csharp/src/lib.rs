//! # Nodex C# frontend
//!
//! Lowers C# (plain .NET and Unity) to the Nodex IR using tree-sitter. It needs no build and no
//! restore: structure and flow graphs come from syntax alone. Semantic enrichment through a
//! Roslyn sidecar arrives in milestone M3.

mod expr;
mod flow;
mod project;
mod structure;
mod syntax;
mod unity;

use std::path::Path;

use nodex_ir::{Diagnostic, FileIr, Language, Severity};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SourceFile, kit::LineIndex};

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
}

/// Lowers C# source text. Never fails: parse problems become diagnostics and `Opaque` nodes.
pub fn lower_source(rel_path: &str, text: &str) -> FileIr {
    let mut parser = tree_sitter::Parser::new();
    let mut ir = FileIr {
        file: rel_path.to_owned(),
        language: Language::Csharp,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
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
    let Some(tree) = parser.parse(text, None) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "parser returned no tree".into(),
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

    let src = syntax::Src {
        text,
        file: rel_path,
        index: &index,
    };
    let mut collector = structure::Collector::new(src);
    collector.compilation_unit(root);
    let structure::Collected {
        mut symbols,
        imports,
        mut type_refs,
        bodies,
    } = collector.finish();

    for job in &bodies {
        let lowered = flow::lower_body(src, job);
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

    ir.symbols = symbols;
    ir.imports = imports;
    ir.type_refs = type_refs;
    ir
}
