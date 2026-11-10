//! Who a call in a flow reaches.
//!
//! Facts come first: a target a semantic provider resolved (`Expr::symbol`, `FlowNode::calls`,
//! `Calls` edges) is exact. Without them the call is resolved from what the code declares — the
//! receiver's declared type (a field, a parameter, a typed local, `this`), the caller's own
//! type, functions of the same module, a unique function name — and is marked *Inferred* with
//! the reason. A call on an interface goes to its only implementation (dependency injection
//! registration or a single implementing type).

use std::collections::HashMap;

use nodex_ir::knowledge::Confidence;
use nodex_ir::{Concept, Expr, FlowGraph, FlowNode, Language, StructEdgeKind, Symbol, SymbolKind};

use crate::flowtree::{CallSite, text};
use crate::index::Index;
use crate::types::{analyze, outer_name, parameter_types};

/// Where a call goes.
#[derive(Clone, Debug)]
pub enum Callee {
    /// A function (or, for a creation, a type) of the project.
    Project {
        target: String,
        confidence: Confidence,
        evidence: String,
        /// The interface the call was written against when it was dispatched to an implementation.
        via: Option<String>,
    },
    /// Library code: `owner` is the receiver's type, or the receiver as written.
    External { owner: String, receiver: String, caps: Vec<&'static str> },
}

/// A receiver's type.
#[derive(Clone, Debug)]
enum TypeRef {
    Project(String),
    External(String),
}

pub struct Resolver<'a> {
    index: &'a Index<'a>,
    types: HashMap<&'a str, Vec<&'a Symbol>>,
    functions: HashMap<&'a str, Vec<&'a Symbol>>,
    bases: HashMap<&'a str, Vec<&'a str>>,
    implementations: HashMap<&'a str, Vec<(&'a str, Option<&'a str>)>>,
    calls: HashMap<&'a str, Vec<&'a str>>,
}

fn is_function(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Method | SymbolKind::Function | SymbolKind::Constructor)
}

/// Names too common to resolve by name alone.
const COMMON: &[&str] = &[
    "map", "then", "get", "set", "add", "push", "pop", "filter", "find", "len", "new", "from", "into", "to_string", "clone", "unwrap", "ToString",
    "Equals", "GetHashCode", "Add", "Remove", "Get", "Set", "Contains", "Dispose", "Invoke", "toString", "run", "init", "update", "apply", "call",
];

impl<'a> Resolver<'a> {
    pub fn new(index: &'a Index<'a>) -> Self {
        let mut types: HashMap<&str, Vec<&Symbol>> = HashMap::new();
        let mut functions: HashMap<&str, Vec<&Symbol>> = HashMap::new();
        for s in index.graph.symbols.iter().filter(|s| !s.external) {
            if s.kind.is_type() {
                types.entry(s.name.as_str()).or_default().push(s);
            } else if is_function(s.kind) {
                functions.entry(s.name.as_str()).or_default().push(s);
            }
        }
        let mut bases: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut implementations: HashMap<&str, Vec<(&str, Option<&str>)>> = HashMap::new();
        let mut calls: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &index.graph.edges {
            match e.kind {
                StructEdgeKind::Inherits | StructEdgeKind::Implements => {
                    bases.entry(e.from.as_str()).or_default().push(e.to.as_str());
                    implementations.entry(e.to.as_str()).or_default().push((e.from.as_str(), None));
                }
                StructEdgeKind::ResolvesDi => {
                    let list = implementations.entry(e.from.as_str()).or_default();
                    list.retain(|(t, _)| *t != e.to.as_str());
                    list.push((e.to.as_str(), e.evidence.as_deref()));
                }
                StructEdgeKind::Calls => calls.entry(e.from.as_str()).or_default().push(e.to.as_str()),
                _ => {}
            }
        }
        Self { index, types, functions, bases, implementations, calls }
    }

    fn name(&self, id: &str) -> String {
        self.index.get(id).map(|s| s.name.clone()).unwrap_or_else(|| id.trim_start_matches("ext:").to_owned())
    }

    /// A project type by name, preferring the caller's namespace.
    pub fn type_named(&self, name: &str, near: &Symbol) -> Option<&'a Symbol> {
        let simple = name.rsplit(['.', ':']).next().unwrap_or(name);
        let list = self.types.get(simple)?;
        if list.len() == 1 {
            return Some(list[0]);
        }
        let ns = self.index.namespace_of(&near.id).map(|n| n.id.as_str());
        list.iter().find(|t| self.index.namespace_of(&t.id).map(|n| n.id.as_str()) == ns).or_else(|| list.iter().find(|t| t.language == near.language)).copied()
    }

    /// A member of a type or of its bases.
    fn member(&self, type_id: &str, name: &str, functions_only: bool) -> Option<&'a Symbol> {
        let mut stack = vec![type_id];
        let mut seen = Vec::new();
        while let Some(t) = stack.pop() {
            if seen.contains(&t) {
                continue;
            }
            seen.push(t);
            let found = self.index.children.get(t).into_iter().flatten().find(|m| {
                m.name == name && (if functions_only { is_function(m.kind) && m.kind != SymbolKind::Constructor } else { matches!(m.kind, SymbolKind::Field | SymbolKind::Property) })
            });
            if let Some(m) = found {
                return Some(m);
            }
            stack.extend(self.bases.get(t).into_iter().flatten().copied());
        }
        None
    }

    fn type_ref(&self, text: &str, near: &Symbol) -> Option<TypeRef> {
        let shape = analyze(text)?;
        if shape.many {
            return Some(TypeRef::External(outer_name(text)));
        }
        match self.type_named(&shape.target, near) {
            Some(t) => Some(TypeRef::Project(t.id.clone())),
            None => Some(TypeRef::External(outer_name(text))),
        }
    }

    /// The type of an expression used as a call receiver.
    fn type_of(&self, e: &Expr, caller: &Symbol, owner: Option<&Symbol>, flow: &FlowGraph, depth: usize) -> Option<(TypeRef, String)> {
        if depth > 4 {
            return None;
        }
        match e.concept {
            Concept::Var => {
                let name = e.label.as_str();
                if matches!(name, "this" | "self" | "Self" | "base" | "super") {
                    return owner.map(|o| (TypeRef::Project(o.id.clone()), format!("`{name}` is {}", o.name)));
                }
                // A local: its declared type, or the type of what initializes it (`var s = new S()`).
                let pin = format!("var:{name}");
                let def = flow.nodes.iter().find(|n| n.outputs.iter().any(|p| p.id == pin));
                if let Some(def) = def {
                    if let Some(ty) = def.outputs.iter().find(|p| p.id == pin).and_then(|p| p.ty.clone()) {
                        return self.type_ref(&ty, caller).map(|t| (t, format!("local `{name}` is {ty}")));
                    }
                    let init = def.expr.as_ref().filter(|_| !matches!(def.concept, Concept::LoopForEach | Concept::LoopRange));
                    if let Some(init) = init
                        && let Some((t, _)) = self.type_of(init, caller, owner, flow, depth + 1)
                    {
                        let what = match &t {
                            TypeRef::Project(id) => self.name(id),
                            TypeRef::External(x) => x.clone(),
                        };
                        return Some((t, format!("local `{name}` holds a {what}")));
                    }
                }
                // A parameter.
                let csharp = caller.language == Language::Csharp;
                if let Some((_, ty)) = caller.signature.as_deref().map(|s| parameter_types(s, &caller.name, csharp)).unwrap_or_default().into_iter().find(|(n, _)| n == name) {
                    return self.type_ref(&ty, caller).map(|t| (t, format!("parameter `{name}` is {ty}")));
                }
                // A field of the caller's type (C# `_repo`, Rust `self.repo` lowered as a var).
                if let Some(o) = owner {
                    if let Some(f) = self.member(&o.id, name, false)
                        && let Some(ty) = &f.type_text
                    {
                        return self.type_ref(ty, caller).map(|t| (t, format!("`{name}` is {ty}")));
                    }
                    // A constructor parameter kept as a property (TypeScript, C# primary constructors).
                    for ctor in self.index.children.get(o.id.as_str()).into_iter().flatten().filter(|m| m.kind == SymbolKind::Constructor) {
                        let params = ctor.signature.as_deref().map(|s| parameter_types(s, &ctor.name, csharp)).unwrap_or_default();
                        if let Some((_, ty)) = params.into_iter().find(|(n, _)| n == name) {
                            return self.type_ref(&ty, caller).map(|t| (t, format!("`{name}` is injected as {ty}")));
                        }
                    }
                }
                // A type name: a static call.
                self.type_named(name, caller).map(|t| (TypeRef::Project(t.id.clone()), format!("static call on {}", t.name)))
            }
            Concept::GetMember => {
                let field = e.label.trim_start_matches('.');
                let inner = e.children.first()?;
                let is_self = inner.concept == Concept::Var && matches!(inner.label.as_str(), "this" | "self");
                let base = if is_self { owner.map(|o| TypeRef::Project(o.id.clone())) } else { self.type_of(inner, caller, owner, flow, depth + 1).map(|(t, _)| t) };
                match base? {
                    TypeRef::Project(t) => {
                        let f = self.member(&t, field, false);
                        if let Some(ty) = f.and_then(|f| f.type_text.as_ref()) {
                            return self.type_ref(ty, caller).map(|r| (r, format!("`{}` is {ty}", text(e))));
                        }
                        // TypeScript parameter properties: `this.repo` from `constructor(private repo: Repo)`.
                        if is_self && let Some(o) = owner {
                            return self.type_of(&Expr { syntax_sites: None, type_info: None, value_info: None, label: field.to_owned(), children: Vec::new(), ..inner.clone() }, caller, Some(o), flow, depth + 1);
                        }
                        None
                    }
                    TypeRef::External(t) => Some((TypeRef::External(t), String::new())),
                }
            }
            Concept::Call => {
                let recv = e.children.iter().find(|c| c.flavors.iter().any(|f| f == "target"))?;
                let (TypeRef::Project(t), _) = self.type_of(recv, caller, owner, flow, depth + 1)? else { return None };
                let m = self.member(&t, &e.label, true)?;
                let ty = m.type_text.as_ref()?;
                self.type_ref(ty, caller).map(|r| (r, format!("{} returns {ty}", m.name)))
            }
            Concept::Construct => {
                let name = e.label.trim_start_matches("new ").trim();
                let name = name.split("::").next().unwrap_or(name);
                self.type_ref(name, caller).map(|t| (t, format!("creates {name}")))
            }
            Concept::Await | Concept::SafeAccess => self.type_of(e.children.first()?, caller, owner, flow, depth + 1),
            _ => None,
        }
    }

    /// Resolves one call site of `node` in `caller`'s flow. `None` when nothing is known.
    pub fn resolve(&self, caller: &Symbol, flow: &FlowGraph, node: &FlowNode, site: &CallSite) -> Option<Callee> {
        let owner = self.index.owner_type(&caller.id).filter(|t| t.id != caller.id);
        // Facts from a semantic provider.
        if let Some(sym) = site.expr.symbol.as_deref().filter(|s| self.index.get(s).is_some()) {
            return Some(self.dispatch(sym.to_owned(), Confidence::Exact, "resolved by the compiler".into()));
        }
        let by_node = node.calls.iter().filter_map(|c| c.symbol.as_deref()).find(|s| self.index.get(s).is_some_and(|t| t.name == site.name));
        if let Some(sym) = by_node {
            return Some(self.dispatch(sym.to_owned(), Confidence::Exact, "resolved by the compiler".into()));
        }
        if site.construct {
            let t = self.type_named(&site.name, caller)?;
            return Some(Callee::Project { target: t.id.clone(), confidence: Confidence::Inferred, evidence: format!("creates {}", t.name), via: None });
        }
        let known: Vec<&str> = self.calls.get(caller.id.as_str()).into_iter().flatten().copied().filter(|t| self.index.get(t).is_some_and(|s| s.name == site.name)).collect();
        if known.len() == 1 {
            return Some(self.dispatch(known[0].to_owned(), Confidence::Exact, "resolved by the compiler".into()));
        }
        match site.receiver {
            None => self.bare(caller, owner, &site.name),
            Some(r) => {
                let written = text(r);
                let typed = self.type_of(r, caller, owner, flow, 0);
                match typed {
                    Some((TypeRef::Project(t), why)) => match self.member(&t, &site.name, true) {
                        Some(m) => Some(self.dispatch(m.id.clone(), Confidence::Inferred, why)),
                        // A project type without the member: inherited from a library type (`DbContext`).
                        None => Some(Callee::External { owner: self.name(&t), caps: caps(&site.name, &written), receiver: written }),
                    },
                    Some((TypeRef::External(t), _)) => Some(Callee::External { owner: t, caps: caps(&site.name, &written), receiver: written }),
                    None => {
                        let root = written.split(['.', '?', '(']).next().unwrap_or(&written).to_owned();
                        Some(Callee::External { owner: root, caps: caps(&site.name, &written), receiver: written })
                    }
                }
            }
        }
    }

    /// A call without a receiver: a method of the caller's type, a function of its module, a
    /// function with that name elsewhere in the project (an import).
    fn bare(&self, caller: &Symbol, owner: Option<&Symbol>, name: &str) -> Option<Callee> {
        if let Some(o) = owner
            && let Some(m) = self.member(&o.id, name, true)
        {
            return Some(self.dispatch(m.id.clone(), Confidence::Inferred, format!("a method of {}", o.name)));
        }
        let list = self.functions.get(name)?;
        let same = |s: &&&Symbol| s.language == caller.language;
        // Nested in the same function, then in the same module or file.
        let parent = caller.parent.as_deref();
        if let Some(f) = list.iter().filter(same).find(|f| f.parent.as_deref() == Some(caller.id.as_str()) || (f.parent.is_some() && f.parent.as_deref() == parent)) {
            return Some(Callee::Project { target: f.id.clone(), confidence: Confidence::Inferred, evidence: "declared next to the caller".into(), via: None });
        }
        let file = caller.origins.first().map(|o| o.file.as_str());
        if let Some(f) = list.iter().filter(same).find(|f| f.origins.first().map(|o| o.file.as_str()) == file && f.kind != SymbolKind::Method) {
            return Some(Callee::Project { target: f.id.clone(), confidence: Confidence::Inferred, evidence: "a function of the same file".into(), via: None });
        }
        if COMMON.contains(&name) {
            return None;
        }
        let free: Vec<&&Symbol> = list.iter().filter(same).filter(|f| f.kind == SymbolKind::Function).collect();
        if free.len() == 1 {
            return Some(Callee::Project { target: free[0].id.clone(), confidence: Confidence::Inferred, evidence: format!("the only function named {name} (imported)"), via: None });
        }
        None
    }

    /// A call on an interface or abstract member goes to its only implementation.
    fn dispatch(&self, target: String, confidence: Confidence, evidence: String) -> Callee {
        let Some(m) = self.index.get(&target) else { return Callee::Project { target, confidence, evidence, via: None } };
        let Some(owner) = self.index.owner_type(&target).filter(|o| o.id != target) else { return Callee::Project { target, confidence, evidence, via: None } };
        let abstract_owner = owner.kind.is_contract() || owner.flavors.iter().any(|f| f == "abstract");
        let impls = self.implementations.get(owner.id.as_str()).cloned().unwrap_or_default();
        if !abstract_owner || impls.len() != 1 {
            return Callee::Project { target, confidence, evidence, via: None };
        }
        let (imp, di) = impls[0];
        match self.member(imp, &m.name, true).filter(|x| x.id != target) {
            Some(x) => {
                let how = match di {
                    Some(ev) => format!("{} is provided by {} ({ev})", owner.name, self.name(imp)),
                    None => format!("{} is the only implementation of {}", self.name(imp), owner.name),
                };
                Callee::Project { target: x.id.clone(), confidence: confidence.weakest(Confidence::Inferred), evidence: format!("{evidence}; {how}"), via: Some(owner.id.clone()) }
            }
            None => Callee::Project { target, confidence, evidence, via: None },
        }
    }
}

fn caps(name: &str, receiver: &str) -> Vec<&'static str> {
    nodex_lang_api::knowledge::call_capabilities(name, receiver)
}
