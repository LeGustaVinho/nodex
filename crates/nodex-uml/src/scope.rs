//! Resolves a [`UmlScope`] to the symbols it covers.

use std::collections::BTreeSet;

use nodex_ir::{Language, Symbol, SymbolKind};

use crate::index::Index;
use crate::{UmlError, UmlScope};

/// The symbols a scope covers.
pub struct Scoped {
    /// Every covered symbol (namespaces, types, members, functions), never external.
    pub symbols: BTreeSet<String>,
    /// Covered types (declared in the scope, or enclosing the selected code).
    pub types: BTreeSet<String>,
    /// Plain-language name of the scope.
    pub title: String,
    /// The scope is a whole project or folder (diagrams summarize more).
    pub broad: bool,
    /// Folder prefix files must have (empty: any).
    pub folder: Option<String>,
    /// Files the scope is limited to (empty: any).
    pub files: BTreeSet<String>,
}

impl Scoped {
    pub fn resolve(index: &Index, scope: &UmlScope) -> Result<Self, UmlError> {
        let all = || index.graph.symbols.iter().filter(|s| !s.external);
        let mut out = Scoped {
            symbols: BTreeSet::new(),
            types: BTreeSet::new(),
            title: String::new(),
            broad: false,
            folder: None,
            files: BTreeSet::new(),
        };
        match scope {
            UmlScope::Project => {
                out.title = "Project".into();
                out.broad = true;
                out.symbols = all().map(|s| s.id.clone()).collect();
            }
            UmlScope::Folder { path } => out.folder(index, path),
            UmlScope::File { path } => {
                out.title = format!("File {path}");
                out.files.insert(path.clone());
                out.symbols = all().filter(|s| s.origins.iter().any(|o| o.file == *path)).map(|s| s.id.clone()).collect();
                if out.symbols.is_empty() {
                    return Err(UmlError::NotFound(format!("file {path}")));
                }
            }
            UmlScope::Range { file, start_byte, end_byte } => {
                let (start, end) = (*start_byte, (*end_byte).max(*start_byte + 1));
                out.title = format!("Selection in {file}");
                out.files.insert(file.clone());
                out.symbols = all()
                    .filter(|s| s.origins.iter().any(|o| o.file == *file && o.start_byte < end && start < o.end_byte))
                    .filter(|s| !matches!(s.kind, SymbolKind::Namespace | SymbolKind::Module) || {
                        // A namespace counts only when the selection covers it entirely.
                        s.origins.iter().any(|o| o.file == *file && start <= o.start_byte && o.end_byte <= end)
                    })
                    .map(|s| s.id.clone())
                    .collect();
                if out.symbols.is_empty() {
                    return Err(UmlError::NotFound(format!("code at {file}:{start_byte}")));
                }
            }
            UmlScope::Symbol { id } => out.symbol(index, id)?,
            UmlScope::Symbols { ids } => {
                let mixed_folders = ids.len() > 1 && ids.iter().any(|id| id.starts_with("dir:"));
                let mut union = BTreeSet::new();
                for id in ids {
                    out.symbol(index, id)?;
                    if mixed_folders {
                        // A directory limits only its own contribution to a union.
                        let folder = out.folder.take();
                        for s in out.symbols.iter().filter_map(|id| index.get(id)) {
                            out.files.extend(s.origins.iter()
                                .filter(|o| in_folder(&o.file, folder.as_deref().unwrap_or("")))
                                .map(|o| o.file.clone()));
                        }
                        union.append(&mut out.symbols);
                    }
                }
                if mixed_folders {
                    out.symbols = union;
                }
                out.title = format!("{} symbols", ids.len());
            }
        }
        // Types: declared in the scope, or enclosing a covered member (a selection inside a method).
        let mut types = BTreeSet::new();
        for id in &out.symbols {
            if let Some(t) = index.owner_type(id).filter(|t| !t.external) {
                types.insert(t.id.clone());
            }
        }
        out.types = types;
        Ok(out)
    }

    fn folder(&mut self, index: &Index, path: &str) {
        let path = path.trim_matches('/');
        self.title = if path.is_empty() { "Project".into() } else { format!("Folder {path}") };
        self.broad = true;
        self.folder = Some(path.to_owned());
        let inside = |s: &Symbol| s.origins.iter().any(|o| in_folder(&o.file, path));
        self.symbols.extend(index.graph.symbols.iter().filter(|s| !s.external && inside(s)).map(|s| s.id.clone()));
    }

    fn symbol(&mut self, index: &Index, id: &str) -> Result<(), UmlError> {
        if let Some(path) = id.strip_prefix("dir:") {
            self.folder(index, path);
            return Ok(());
        }
        if let Some(ns) = id.strip_prefix("grp:ns:") {
            self.title = format!("Namespace {ns}");
            self.broad = true;
            self.namespace(index, ns, None);
            return Ok(());
        }
        let s = index.get(id).filter(|s| !s.external).ok_or_else(|| UmlError::NotFound(format!("symbol {id}")))?;
        self.title = format!("{:?} {}", s.kind, s.name);
        if matches!(s.kind, SymbolKind::Namespace | SymbolKind::Module | SymbolKind::Package) {
            self.broad = true;
        }
        if s.kind == SymbolKind::Namespace {
            self.namespace(index, &s.qualified_name, Some(s.language));
        }
        let mut stack = vec![s];
        while let Some(cur) = stack.pop() {
            self.symbols.insert(cur.id.clone());
            if let Some(kids) = index.children.get(cur.id.as_str()) {
                stack.extend(kids.iter().copied());
            }
        }
        Ok(())
    }

    fn namespace(&mut self, index: &Index, name: &str, language: Option<Language>) {
        for s in index.graph.symbols.iter().filter(|s| !s.external) {
            let namespace = if matches!(s.kind, SymbolKind::Namespace | SymbolKind::Module) {
                Some(s)
            } else {
                index.namespace_of(&s.id)
            };
            if namespace.is_some_and(|n| {
                if language.is_some_and(|l| n.kind != SymbolKind::Namespace || n.language != l) {
                    return false;
                }
                let q = n.qualified_name.as_str();
                q == name || q.strip_prefix(name).is_some_and(|r| r.starts_with('.') || r.starts_with("::") || r.starts_with('/'))
            }) {
                self.symbols.insert(s.id.clone());
            }
        }
    }

    /// The scope is a file, a selection or a single symbol: diagrams show details.
    pub fn narrow(&self) -> bool {
        !self.broad
    }
}

fn in_folder(file: &str, folder: &str) -> bool {
    folder.is_empty() || file.strip_prefix(folder).is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_ir::StructureGraph;
    use nodex_ir::knowledge::ProjectKnowledge;

    #[test]
    fn real_namespace_includes_subnamespaces_without_structural_parents() {
        let ir = nodex_lang_csharp::lower_source(
            "Test.cs",
            "namespace Shop { class Root {} } namespace Shop.Tools { class Tool {} } namespace Shopper { class Other {} }",
        );
        let graph = StructureGraph { symbols: ir.symbols, edges: Vec::new() };
        let knowledge = ProjectKnowledge::default();
        let index = Index::new(&graph, &knowledge);
        assert!(index.get("ns:Shop.Tools").unwrap().parent.is_none());
        let scope = Scoped::resolve(&index, &UmlScope::Symbol { id: "ns:Shop".into() }).unwrap();
        assert!(scope.symbols.contains("ns:Shop.Tools"));
        assert_eq!(scope.types, BTreeSet::from(["Shop.Root".into(), "Shop.Tools.Tool".into()]));
    }
}
