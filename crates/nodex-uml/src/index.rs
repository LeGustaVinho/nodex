//! Lookups over the structure graph and the knowledge graph shared by every diagram.

use std::collections::HashMap;

use nodex_ir::knowledge::{DataModel, ProjectKnowledge, SymbolInfo};
use nodex_ir::{Language, StructEdge, StructEdgeKind, StructureGraph, Symbol, SymbolKind};

pub struct Index<'a> {
    pub graph: &'a StructureGraph,
    pub knowledge: &'a ProjectKnowledge,
    pub by_id: HashMap<&'a str, &'a Symbol>,
    pub children: HashMap<&'a str, Vec<&'a Symbol>>,
    pub info: HashMap<&'a str, &'a SymbolInfo>,
    pub models: HashMap<&'a str, &'a DataModel>,
    /// Types with routes among their members (or that are route handlers).
    pub routed: HashMap<String, usize>,
}

impl<'a> Index<'a> {
    pub fn new(graph: &'a StructureGraph, knowledge: &'a ProjectKnowledge) -> Self {
        let by_id: HashMap<&str, &Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut children: HashMap<&str, Vec<&Symbol>> = HashMap::new();
        for s in &graph.symbols {
            if let Some(p) = &s.parent {
                children.entry(p.as_str()).or_default().push(s);
            }
        }
        for list in children.values_mut() {
            list.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let info = knowledge.symbols.iter().map(|i| (i.symbol.as_str(), i)).collect();
        let models = knowledge.models.iter().map(|m| (m.symbol.as_str(), &m.model)).collect();
        let mut index = Self { graph, knowledge, by_id, children, info, models, routed: HashMap::new() };
        let mut routed: HashMap<String, usize> = HashMap::new();
        for r in &knowledge.routes {
            // Only route handlers that are methods make their type a controller (a Minimal API
            // lambda belongs to the program, not to a controller).
            let is_route = index.info.get(r.handler.as_str()).and_then(|i| i.entry.as_ref()).is_some_and(|e| e.kind == nodex_ir::knowledge::EntryKind::Route);
            let is_method = index.get(&r.handler).is_some_and(|s| s.kind == SymbolKind::Method);
            if !(is_route && is_method) {
                continue;
            }
            let owner = index.owner_type(&r.handler).map(|t| t.id.clone()).unwrap_or_else(|| r.handler.clone());
            *routed.entry(owner).or_default() += 1;
        }
        index.routed = routed;
        index
    }

    pub fn get(&self, id: &str) -> Option<&'a Symbol> {
        self.by_id.get(id).copied()
    }

    /// The type a symbol belongs to (itself for a type).
    pub fn owner_type(&self, id: &str) -> Option<&'a Symbol> {
        let mut cur = self.get(id)?;
        loop {
            if cur.kind.is_type() {
                return Some(cur);
            }
            cur = self.get(cur.parent.as_deref()?)?;
        }
    }

    /// The namespace or module a symbol is declared in.
    pub fn namespace_of(&self, id: &str) -> Option<&'a Symbol> {
        let mut cur = self.get(self.get(id)?.parent.as_deref()?)?;
        loop {
            if matches!(cur.kind, SymbolKind::Namespace | SymbolKind::Module) {
                return Some(cur);
            }
            cur = self.get(cur.parent.as_deref()?)?;
        }
    }

    pub fn file_of(&self, id: &str) -> Option<&'a str> {
        self.get(id)?.origins.first().map(|o| o.file.as_str())
    }

    pub fn is_test(&self, id: &str) -> bool {
        if self.info.get(id).is_some_and(|i| i.test) {
            return true;
        }
        // A member of a test class, a function of a test module.
        let mut cur = self.get(id).and_then(|s| s.parent.as_deref());
        while let Some(p) = cur {
            if self.info.get(p).is_some_and(|i| i.test) {
                return true;
            }
            cur = self.get(p).and_then(|s| s.parent.as_deref());
        }
        false
    }

    /// Relations except containment, of the kinds a diagram lifts to types or packages.
    pub fn relations(&self) -> impl Iterator<Item = &'a StructEdge> {
        self.graph.edges.iter().filter(|e| e.kind != StructEdgeKind::Contains)
    }

    /// The lifetime a DI registration gives an implementation (`scoped`), read from evidence.
    pub fn di_lifetime(&self, type_id: &str) -> Option<String> {
        self.graph.edges.iter().filter(|e| e.kind == StructEdgeKind::ResolvesDi && e.to == type_id).find_map(|e| {
            let ev = e.evidence.as_deref()?;
            let rest = ev.strip_prefix("registered as ")?;
            Some(rest.split_whitespace().next()?.to_owned())
        })
    }
}

/// The package a symbol belongs to for diagrams: its namespace or module, with TypeScript files
/// grouped by folder (one module per file is too fine).
pub fn package_key(index: &Index, s: &nodex_ir::Symbol) -> (String, &'static str) {
    if s.language == Language::Typescript {
        let file = s.origins.first().map(|o| o.file.as_str()).unwrap_or("");
        return (dirname(file).to_owned(), "/");
    }
    match index.namespace_of(&s.id) {
        Some(ns) if s.language == Language::Rust => (ns.qualified_name.clone(), "::"),
        Some(ns) => (ns.qualified_name.clone(), "."),
        None => (String::new(), "."),
    }
}

pub fn dirname(file: &str) -> &str {
    file.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}
