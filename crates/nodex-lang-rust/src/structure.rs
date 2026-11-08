//! Declarations → structure symbols, imports and type references.
//!
//! Module paths follow Rust: `src/main.rs` / `src/lib.rs` are the crate root (`ns:<crate>`),
//! `src/orders/mod.rs` and `src/orders.rs` are `ns:<crate>::orders`, `mod x { … }` nests. Items are
//! `<module>::<Name>`, members `<module>::<Type>::<member>`. The crate name comes from the
//! directory that holds `src/` (`crates/nodex-ir/src/lib.rs` → `nodex_ir`), or `crate` for a
//! package at the project root.

use std::collections::HashMap;

use nodex_ir::{Language, Symbol, SymbolId, SymbolKind, TypeRef, TypeRelation};
use tree_sitter::Node;

use crate::syntax::{
    Src, attribute_flavors, bare_type_name, children_of_kind, doc_comment, first_child_of_kind, function_modifiers, last_segment,
    named_children, parameters, receiver, type_names, visibility,
};

/// What kind of body a [`BodyJob`] lowers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Function,
    Method,
    /// A `fn` declared inside another function, or a block-bodied closure handed to a call.
    LocalFunction,
}

/// A function body waiting to be lowered into a flow graph.
pub struct BodyJob<'t> {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: BodyKind,
    /// The function or closure node.
    pub decl: Node<'t>,
    /// The `block` (or a closure's expression body).
    pub body: Node<'t>,
    pub signature: String,
    pub params: Vec<(String, Option<String>)>,
    /// The function declares a return type: its tail expression is what it returns.
    pub returns_value: bool,
    pub flavors: Vec<String>,
}

pub struct Collected<'t> {
    pub symbols: Vec<Symbol>,
    /// Import targets: module/item paths (`sample::orders::Order`) or `pkg:<crate>`.
    pub imports: Vec<String>,
    pub type_refs: Vec<TypeRef>,
    pub bodies: Vec<BodyJob<'t>>,
    /// Start byte of a function or closure with its own Blueprint → its symbol.
    pub function_ids: HashMap<usize, SymbolId>,
    /// Qualified name of the file's module (`sample::orders`).
    pub module: String,
}

/// Crates whose paths are never project modules.
const STD_CRATES: &[&str] = &["std", "core", "alloc", "proc_macro", "test"];

/// `(crate name, module segments)` of a project file.
pub fn module_of(rel: &str) -> (String, Vec<String>) {
    let segs: Vec<&str> = rel.split('/').collect();
    let crate_name = |i: usize| -> String {
        if i == 0 { "crate".to_owned() } else { segs[i - 1].replace('-', "_") }
    };
    let strip = |s: &str| s.strip_suffix(".rs").unwrap_or(s).to_owned();
    if let Some(i) = segs.iter().position(|s| *s == "src") {
        let mut module: Vec<String> = segs[i + 1..].iter().map(|s| strip(s)).collect();
        if module.len() == 1 && matches!(module[0].as_str(), "main" | "lib") {
            module.clear();
        } else if module.last().is_some_and(|l| l == "mod") {
            module.pop();
        }
        return (crate_name(i), module);
    }
    if let Some(j) = segs.iter().position(|s| matches!(*s, "tests" | "examples" | "benches")) {
        let mut module: Vec<String> = segs[j..].iter().map(|s| strip(s)).collect();
        if module.last().is_some_and(|l| l == "mod" || l == "main") {
            module.pop();
        }
        return (crate_name(j), module);
    }
    let crate_name = if segs.len() > 1 { segs[segs.len() - 2].replace('-', "_") } else { "crate".to_owned() };
    let module = vec![strip(segs[segs.len() - 1])];
    (crate_name, module)
}

/// The qualified module name of a file: `sample::orders`.
pub fn module_path(rel: &str) -> String {
    let (krate, segs) = module_of(rel);
    std::iter::once(krate).chain(segs).collect::<Vec<_>>().join("::")
}

pub struct Collector<'a, 't> {
    src: Src<'a>,
    krate: String,
    module: String,
    symbols: Vec<Symbol>,
    type_refs: Vec<TypeRef>,
    bodies: Vec<BodyJob<'t>>,
    imports: Vec<String>,
    function_ids: HashMap<usize, SymbolId>,
    /// Modules declared with `mod x;` / `mod x { }` in this file, by enclosing module.
    declared_mods: Vec<String>,
    /// Types declared in this file (qualified), so `impl` blocks attach to them.
    types: HashMap<String, SymbolId>,
}

impl<'a, 't> Collector<'a, 't> {
    pub fn new(src: Src<'a>) -> Self {
        let (krate, _) = module_of(src.file);
        Self {
            src,
            krate,
            module: module_path(src.file),
            symbols: Vec::new(),
            type_refs: Vec::new(),
            bodies: Vec::new(),
            imports: Vec::new(),
            function_ids: HashMap::new(),
            declared_mods: Vec::new(),
            types: HashMap::new(),
        }
    }

    pub fn finish(self) -> Collected<'t> {
        Collected {
            symbols: self.symbols,
            imports: self.imports,
            type_refs: self.type_refs,
            bodies: self.bodies,
            function_ids: self.function_ids,
            module: self.module,
        }
    }

    fn sym(&self, id: &str, kind: SymbolKind, name: &str, parent: Option<&str>, node: Node, flavors: Vec<String>) -> Symbol {
        Symbol {
            id: id.to_owned(),
            kind,
            name: name.to_owned(),
            qualified_name: id.trim_start_matches("ns:").to_owned(),
            parent: parent.map(str::to_owned),
            language: Language::Rust,
            origins: vec![self.src.span(node)],
            flavors,
            signature: None,
            type_text: None,
            has_flow: false,
            external: false,
            doc: doc_comment(self.src, node),
        }
    }

    fn push(&mut self, s: Symbol) {
        match self.symbols.iter_mut().find(|x| x.id == s.id) {
            Some(existing) => {
                for o in s.origins {
                    if !existing.origins.contains(&o) {
                        existing.origins.push(o);
                    }
                }
                for f in s.flavors {
                    if !existing.flavors.contains(&f) {
                        existing.flavors.push(f);
                    }
                }
            }
            None => self.symbols.push(s),
        }
    }

    /// An id not used yet in this file: `x`, `x~2`, ...
    fn unique(&self, id: String) -> String {
        if !self.symbols.iter().any(|s| s.id == id) {
            return id;
        }
        (2..).map(|i| format!("{id}~{i}")).find(|c| !self.symbols.iter().any(|s| &s.id == c)).unwrap_or(id)
    }

    pub fn source_file(&mut self, root: Node<'t>) {
        let src = self.src;
        let (_, segs) = module_of(src.file);
        let name = segs.last().cloned().unwrap_or_else(|| self.krate.clone());
        let parent = if segs.is_empty() {
            None
        } else {
            Some(format!("ns:{}", std::iter::once(self.krate.clone()).chain(segs[..segs.len() - 1].iter().cloned()).collect::<Vec<_>>().join("::")))
        };
        let mut flavors = vec![];
        if segs.is_empty() {
            flavors.push("crate-root".to_owned());
        }
        let module_id = format!("ns:{}", self.module);
        let mut s = self.sym(&module_id, SymbolKind::Module, &name, parent.as_deref(), root, flavors);
        // Inner docs (`//!`) describe the module.
        s.doc = inner_doc(src, root);
        self.push(s);
        let module = self.module.clone();
        self.items(root, &module);
    }

    fn items(&mut self, container: Node<'t>, module: &str) {
        for item in named_children(container) {
            self.item(item, module);
        }
    }

    fn item(&mut self, item: Node<'t>, module: &str) {
        let src = self.src;
        let module_id = format!("ns:{module}");
        let mut flavors = attribute_flavors(src, item);
        if let Some(v) = visibility(src, item) {
            flavors.push(v);
        }
        match item.kind() {
            "mod_item" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                self.declared_mods.push(name.clone());
                if let Some(body) = item.child_by_field_name("body") {
                    let path = format!("{module}::{name}");
                    flavors.push("inline-module".into());
                    let s = self.sym(&format!("ns:{path}"), SymbolKind::Module, &name, Some(&module_id), item, flavors);
                    self.push(s);
                    self.items(body, &path);
                }
            }
            "struct_item" | "union_item" => self.struct_item(item, module, flavors),
            "enum_item" => self.enum_item(item, module, flavors),
            "trait_item" => self.trait_item(item, module, flavors),
            "impl_item" => self.impl_item(item, module, flavors),
            "function_item" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                let id = self.unique(format!("{module}::{name}"));
                self.function(item, &id, &name, &module_id, SymbolKind::Function, BodyKind::Function, flavors);
            }
            "function_signature_item" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                flavors.extend(function_modifiers(src, item));
                let mut s = self.sym(&format!("{module}::{name}"), SymbolKind::Function, &name, Some(&module_id), item, flavors);
                s.signature = Some(src.norm(item).trim_end_matches(';').to_owned());
                self.push(s);
            }
            "type_item" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                let id = format!("{module}::{name}");
                let mut s = self.sym(&id, SymbolKind::TypeAlias, &name, Some(&module_id), item, flavors);
                s.signature = Some(src.norm(item).trim_end_matches(';').to_owned());
                s.type_text = item.child_by_field_name("type").map(|t| src.norm(t));
                self.push(s);
                self.refs_of_type(&id, item.child_by_field_name("type"));
            }
            "const_item" | "static_item" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                flavors.push(if item.kind() == "const_item" { "const".into() } else { "static".into() });
                if first_child_of_kind(item, "mutable_specifier").is_some() {
                    flavors.push("mut".into());
                }
                let id = format!("{module}::{name}");
                let mut s = self.sym(&id, SymbolKind::Field, &name, Some(&module_id), item, flavors);
                s.signature = Some(src.header(item, item.child_by_field_name("value")).trim_end_matches('=').trim().to_owned());
                s.type_text = item.child_by_field_name("type").map(|t| src.norm(t));
                self.push(s);
                self.refs_of_type(&id, item.child_by_field_name("type"));
            }
            "macro_definition" => {
                let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
                flavors.push("macro".into());
                let mut s = self.sym(&format!("{module}::{name}!"), SymbolKind::Function, &format!("{name}!"), Some(&module_id), item, flavors);
                s.signature = Some(format!("macro_rules! {name}"));
                self.push(s);
            }
            "use_declaration" => {
                if let Some(arg) = item.child_by_field_name("argument") {
                    let mut paths = Vec::new();
                    use_paths(src, arg, "", &mut paths);
                    for p in paths {
                        if let Some(t) = self.import_target(module, &p)
                            && !self.imports.contains(&t)
                        {
                            self.imports.push(t);
                        }
                    }
                }
            }
            "extern_crate_declaration" => {
                if let Some(n) = item.child_by_field_name("name") {
                    let t = format!("pkg:{}", src.norm(n));
                    if !self.imports.contains(&t) {
                        self.imports.push(t);
                    }
                }
            }
            "foreign_mod_item" => {
                if let Some(body) = item.child_by_field_name("body") {
                    for f in named_children(body) {
                        if f.kind() == "function_signature_item"
                            && let Some(name) = f.child_by_field_name("name").map(|n| src.norm(n))
                        {
                            let mut s = self.sym(&format!("{module}::{name}"), SymbolKind::Function, &name, Some(&module_id), f, vec!["extern".into()]);
                            s.signature = Some(src.norm(f).trim_end_matches(';').to_owned());
                            self.push(s);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Resolves a `use` path written in `module` to an import target.
    fn import_target(&self, module: &str, path: &str) -> Option<String> {
        let mut segs: Vec<&str> = path.split("::").filter(|s| !s.is_empty()).collect();
        if segs.last() == Some(&"self") {
            segs.pop();
        }
        let first = *segs.first()?;
        let module_segs: Vec<&str> = module.split("::").collect();
        let resolved: Vec<String> = match first {
            "crate" => std::iter::once(self.krate.clone()).chain(segs[1..].iter().map(|s| (*s).to_owned())).collect(),
            "self" => module_segs.iter().map(|s| (*s).to_owned()).chain(segs[1..].iter().map(|s| (*s).to_owned())).collect(),
            "super" => {
                let mut base: Vec<String> = module_segs.iter().map(|s| (*s).to_owned()).collect();
                let mut rest = &segs[..];
                while rest.first() == Some(&"super") {
                    base.pop();
                    rest = &rest[1..];
                }
                base.into_iter().chain(rest.iter().map(|s| (*s).to_owned())).collect()
            }
            _ if self.declared_mods.iter().any(|m| m == first) => {
                module_segs.iter().map(|s| (*s).to_owned()).chain(segs.iter().map(|s| (*s).to_owned())).collect()
            }
            _ if STD_CRATES.contains(&first) => return Some(format!("pkg:{first}")),
            _ => return Some(format!("pkg:{first}")),
        };
        (!resolved.is_empty()).then(|| resolved.join("::"))
    }

    fn struct_item(&mut self, item: Node<'t>, module: &str, mut flavors: Vec<String>) {
        let src = self.src;
        let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
        let id = format!("{module}::{name}");
        if item.kind() == "union_item" {
            flavors.push("c-union".into());
        }
        let body = item.child_by_field_name("body");
        match body.map(|b| b.kind()) {
            Some("ordered_field_declaration_list") => flavors.push("tuple-struct".into()),
            None => flavors.push("unit-struct".into()),
            _ => {}
        }
        let mut s = self.sym(&id, SymbolKind::Struct, &name, Some(&format!("ns:{module}")), item, flavors);
        s.signature = Some(src.header(item, body).trim_end_matches(';').to_owned());
        self.push(s);
        self.types.insert(name.clone(), id.clone());
        if let Some(body) = body {
            self.fields(body, &id);
        }
    }

    fn fields(&mut self, body: Node<'t>, owner: &str) {
        let src = self.src;
        match body.kind() {
            "field_declaration_list" => {
                for f in children_of_kind(body, "field_declaration") {
                    let Some(name) = f.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
                    let mut flavors = attribute_flavors(src, f);
                    if let Some(v) = visibility(src, f) {
                        flavors.push(v);
                    }
                    let fid = format!("{owner}::{name}");
                    let mut s = self.sym(&fid, SymbolKind::Field, &name, Some(owner), f, flavors);
                    s.type_text = f.child_by_field_name("type").map(|t| src.norm(t));
                    s.signature = Some(src.norm(f));
                    self.push(s);
                    self.refs_of_type(owner, f.child_by_field_name("type"));
                }
            }
            "ordered_field_declaration_list" => {
                let types: Vec<Node> = named_children(body).into_iter().filter(|c| c.kind() != "visibility_modifier" && c.kind() != "attribute_item").collect();
                for (i, t) in types.into_iter().enumerate() {
                    let fid = format!("{owner}::{i}");
                    let mut s = self.sym(&fid, SymbolKind::Field, &i.to_string(), Some(owner), t, Vec::new());
                    s.type_text = Some(src.norm(t));
                    self.push(s);
                    self.refs_of_type(owner, Some(t));
                }
            }
            _ => {}
        }
    }

    fn enum_item(&mut self, item: Node<'t>, module: &str, flavors: Vec<String>) {
        let src = self.src;
        let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
        let id = format!("{module}::{name}");
        let body = item.child_by_field_name("body");
        let variants: Vec<Node> = body.map(|b| children_of_kind(b, "enum_variant")).unwrap_or_default();
        let with_data = variants.iter().any(|v| v.child_by_field_name("body").is_some());
        let kind = if with_data { SymbolKind::Union } else { SymbolKind::Enum };
        let mut s = self.sym(&id, kind, &name, Some(&format!("ns:{module}")), item, flavors);
        s.signature = Some(src.header(item, body));
        self.push(s);
        self.types.insert(name.clone(), id.clone());
        for v in variants {
            let Some(vname) = v.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
            let vid = format!("{id}::{vname}");
            let mut vs = self.sym(&vid, SymbolKind::EnumMember, &vname, Some(&id), v, attribute_flavors(src, v));
            vs.signature = Some(src.norm(v));
            self.push(vs);
            if let Some(b) = v.child_by_field_name("body") {
                self.fields(b, &vid);
            }
        }
    }

    fn trait_item(&mut self, item: Node<'t>, module: &str, mut flavors: Vec<String>) {
        let src = self.src;
        let Some(name) = item.child_by_field_name("name").map(|n| src.norm(n)) else { return };
        let id = format!("{module}::{name}");
        if item.child(0).is_some_and(|c| src.text(c) == "unsafe") {
            flavors.push("unsafe".into());
        }
        let body = item.child_by_field_name("body");
        let mut s = self.sym(&id, SymbolKind::Trait, &name, Some(&format!("ns:{module}")), item, flavors);
        s.signature = Some(src.header(item, body));
        self.push(s);
        self.types.insert(name.clone(), id.clone());
        // Supertraits: `trait Shape: Display + Debug`.
        if let Some(bounds) = item.child_by_field_name("bounds") {
            let mut names = Vec::new();
            type_names(src, bounds, &mut names);
            for n in names {
                self.type_refs.push(TypeRef { from: id.clone(), name: n, relation: TypeRelation::Base });
            }
        }
        if let Some(body) = body {
            self.members(body, &id, true, None);
        }
    }

    fn impl_item(&mut self, item: Node<'t>, module: &str, mut flavors: Vec<String>) {
        let src = self.src;
        let Some(ty) = item.child_by_field_name("type") else { return };
        let ty_name = bare_type_name(&src.norm(ty));
        let trait_node = item.child_by_field_name("trait");
        // Methods merge into the type declared in this file, or a placeholder for it.
        let owner = match self.types.get(&ty_name) {
            Some(id) => id.clone(),
            None => {
                let id = format!("{module}::{ty_name}");
                if !self.symbols.iter().any(|s| s.id == id) {
                    let mut s = self.sym(&id, SymbolKind::Struct, &ty_name, Some(&format!("ns:{module}")), item, vec!["impl-only".into()]);
                    s.doc = None;
                    s.signature = Some(src.header(item, item.child_by_field_name("body")));
                    self.push(s);
                }
                id
            }
        };
        if item.child(0).is_some_and(|c| src.text(c) == "unsafe") {
            flavors.push("unsafe".into());
        }
        let trait_name = trait_node.map(|t| src.norm(t));
        if let Some(t) = trait_node {
            let negative = src.text(item).contains(&format!("!{}", src.text(t)));
            let base = bare_type_name(&src.norm(t));
            if !negative {
                self.type_refs.push(TypeRef { from: owner.clone(), name: base.clone(), relation: TypeRelation::Base });
                if let Some(s) = self.symbols.iter_mut().find(|s| s.id == owner) {
                    let f = format!("impl:{base}");
                    if !s.flavors.contains(&f) {
                        s.flavors.push(f);
                    }
                }
            }
        }
        if let Some(body) = item.child_by_field_name("body") {
            let mut extra = flavors;
            if let Some(t) = &trait_name {
                extra.push(format!("from-trait:{}", bare_type_name(t)));
            }
            self.members(body, &owner, false, Some(extra));
        }
    }

    /// Members of a trait or `impl` block.
    fn members(&mut self, body: Node<'t>, owner: &str, in_trait: bool, extra: Option<Vec<String>>) {
        let src = self.src;
        for m in named_children(body) {
            let mut flavors = attribute_flavors(src, m);
            if let Some(v) = visibility(src, m) {
                flavors.push(v);
            }
            flavors.extend(extra.clone().unwrap_or_default());
            match m.kind() {
                "function_item" => {
                    let Some(name) = m.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
                    if in_trait {
                        flavors.push("default".into());
                    }
                    let id = self.unique(format!("{owner}::{name}"));
                    self.function(m, &id, &name, owner, SymbolKind::Method, BodyKind::Method, flavors);
                }
                "function_signature_item" => {
                    let Some(name) = m.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
                    flavors.push("required".into());
                    flavors.extend(function_modifiers(src, m));
                    match receiver(src, m) {
                        Some(r) => flavors.push(r),
                        None => flavors.push("static".into()),
                    }
                    let mut s = self.sym(&format!("{owner}::{name}"), SymbolKind::Method, &name, Some(owner), m, flavors);
                    s.signature = Some(src.norm(m).trim_end_matches(';').to_owned());
                    s.type_text = m.child_by_field_name("return_type").map(|t| src.norm(t));
                    self.push(s);
                }
                "const_item" => {
                    let Some(name) = m.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
                    flavors.push("const".into());
                    let mut s = self.sym(&format!("{owner}::{name}"), SymbolKind::Field, &name, Some(owner), m, flavors);
                    s.signature = Some(src.norm(m).trim_end_matches(';').to_owned());
                    s.type_text = m.child_by_field_name("type").map(|t| src.norm(t));
                    self.push(s);
                }
                "type_item" | "associated_type" => {
                    let Some(name) = m.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
                    flavors.push("associated-type".into());
                    let mut s = self.sym(&format!("{owner}::{name}"), SymbolKind::TypeAlias, &name, Some(owner), m, flavors);
                    s.signature = Some(src.norm(m).trim_end_matches(';').to_owned());
                    self.push(s);
                }
                "macro_invocation" => {}
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn function(&mut self, func: Node<'t>, id: &str, name: &str, parent: &str, kind: SymbolKind, body_kind: BodyKind, mut flavors: Vec<String>) {
        let src = self.src;
        flavors.extend(function_modifiers(src, func));
        if kind == SymbolKind::Method {
            match receiver(src, func) {
                Some(r) => flavors.push(r),
                None => flavors.push("static".into()),
            }
        }
        if func.child_by_field_name("type_parameters").is_some() || first_child_of_kind(func, "where_clause").is_some() {
            flavors.push("generic".into());
        }
        if body_kind == BodyKind::LocalFunction && !flavors.iter().any(|f| f == "local-function") {
            flavors.push("local-function".into());
        }
        let body = func.child_by_field_name("body");
        let returns = func.child_by_field_name("return_type");
        let signature = src.header(func, body);
        let mut s = self.sym(id, kind, name, Some(parent), func, flavors.clone());
        s.signature = Some(signature.clone());
        s.type_text = returns.map(|t| src.norm(t));
        self.push(s);
        self.refs_of_type(id, returns);
        if let Some(list) = func.child_by_field_name("parameters") {
            for p in children_of_kind(list, "parameter") {
                self.refs_of_type(id, p.child_by_field_name("type"));
            }
        }
        let Some(body) = body else { return };
        self.function_ids.insert(func.start_byte(), id.to_owned());
        self.local_functions(body, id);
        let returns_value = returns.is_some_and(|t| !matches!(src.norm(t).as_str(), "()" | "!"));
        self.bodies.push(BodyJob {
            symbol: id.to_owned(),
            name: name.to_owned(),
            kind: body_kind,
            decl: func,
            body,
            signature,
            params: parameters(src, func),
            returns_value,
            flavors,
        });
    }

    /// Functions declared inside a body (`fn helper() {}`), block-bodied closures handed to a
    /// call (`items.for_each(|x| { … })`), and items declared locally. Each function gets a
    /// Blueprint of its own.
    fn local_functions(&mut self, body: Node<'t>, owner: &str) {
        let src = self.src;
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            for c in named_children(n) {
                match c.kind() {
                    "function_item" => {
                        let name = c.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
                        let id = self.unique(format!("{owner}::{name}"));
                        let flavors = attribute_flavors(src, c);
                        self.function(c, &id, &name, owner, SymbolKind::Function, BodyKind::LocalFunction, flavors);
                    }
                    "closure_expression" => {
                        if let Some(callee) = callback_callee(src, c)
                            && c.child_by_field_name("body").is_some_and(|b| b.kind() == "block" && named_children(b).iter().any(|s| !crate::syntax::is_comment(s.kind())))
                        {
                            let name = format!("{callee} callback");
                            let id = self.unique(format!("{owner}::{name}"));
                            self.closure(c, &id, &name, owner);
                        } else {
                            stack.push(c);
                        }
                    }
                    "struct_item" | "enum_item" | "trait_item" | "impl_item" | "union_item" | "type_item" | "const_item"
                    | "static_item" | "mod_item" | "macro_definition" | "use_declaration" => {
                        let module = owner.to_owned();
                        self.item(c, &module);
                    }
                    _ => stack.push(c),
                }
            }
        }
    }

    fn closure(&mut self, c: Node<'t>, id: &str, name: &str, owner: &str) {
        let src = self.src;
        let Some(body) = c.child_by_field_name("body") else { return };
        let mut flavors = vec!["local-function".to_owned(), "callback".to_owned()];
        let head = src.header(c, Some(body));
        if head.starts_with("move") || head.contains(" move ") {
            flavors.push("move".into());
        }
        if head.starts_with("async") {
            flavors.push("async".into());
        }
        let mut s = self.sym(id, SymbolKind::Function, name, Some(owner), c, flavors.clone());
        s.signature = Some(head.clone());
        s.doc = None;
        self.push(s);
        self.function_ids.insert(c.start_byte(), id.to_owned());
        self.local_functions(body, id);
        self.bodies.push(BodyJob {
            symbol: id.to_owned(),
            name: name.to_owned(),
            kind: BodyKind::LocalFunction,
            decl: c,
            body,
            signature: head,
            params: parameters(src, c),
            // A closure's final expression (without `;`) is the value it gives back.
            returns_value: true,
            flavors,
        });
    }

    fn refs_of_type(&mut self, from: &str, ty: Option<Node>) {
        let Some(ty) = ty else { return };
        let mut names = Vec::new();
        type_names(self.src, ty, &mut names);
        for n in names {
            if matches!(n.as_str(), "Self" | "Option" | "Result" | "Vec" | "String" | "Box" | "Rc" | "Arc" | "HashMap" | "HashSet") {
                continue;
            }
            let r = TypeRef { from: from.to_owned(), name: n, relation: TypeRelation::References };
            if !self.type_refs.iter().any(|x| x.from == r.from && x.name == r.name && x.relation == r.relation) {
                self.type_refs.push(r);
            }
        }
    }
}

/// The method or function a closure is passed to (`items.for_each(|x| …)` → `for_each`), or
/// `None` when the closure is not a call argument.
pub fn callback_callee(src: Src, closure: Node) -> Option<String> {
    let args = closure.parent().filter(|p| p.kind() == "arguments")?;
    let mut call = args.parent().filter(|p| p.kind() == "call_expression")?;
    // `on_shipped(Box::new(|o| …))`: the wrapper is not who calls it.
    while call.child_by_field_name("function").is_some_and(|f| {
        matches!(src.norm(f).as_str(), "Box::new" | "Rc::new" | "Arc::new" | "Some" | "Box::pin" | "std::boxed::Box::new")
    }) && let Some(outer) = call.parent().filter(|p| p.kind() == "arguments").and_then(|a| a.parent()).filter(|c| c.kind() == "call_expression")
    {
        call = outer;
    }
    let f = call.child_by_field_name("function")?;
    let name = match f.kind() {
        "field_expression" => f.child_by_field_name("field").map(|n| src.norm(n))?,
        "identifier" => src.norm(f),
        "scoped_identifier" => last_segment(&src.norm(f)).to_owned(),
        "generic_function" => f.child_by_field_name("function").map(|g| last_segment(&src.norm(g)).to_owned())?,
        _ => "call".to_owned(),
    };
    Some(if name.is_empty() { "call".into() } else { name })
}

/// Paths named by a `use` tree: `a::{b, c::d as e, f::*}` → `a::b`, `a::c::d`, `a::f`.
fn use_paths(src: Src, node: Node, prefix: &str, out: &mut Vec<String>) {
    let join = |p: &str, s: &str| if p.is_empty() { s.to_owned() } else { format!("{p}::{s}") };
    match node.kind() {
        "scoped_use_list" => {
            let path = node.child_by_field_name("path").map(|p| src.norm(p)).unwrap_or_default();
            let base = join(prefix, &path);
            if let Some(list) = node.child_by_field_name("list") {
                for c in named_children(list) {
                    use_paths(src, c, &base, out);
                }
            }
        }
        "use_list" => {
            for c in named_children(node) {
                use_paths(src, c, prefix, out);
            }
        }
        "use_as_clause" => {
            if let Some(p) = node.child_by_field_name("path") {
                out.push(join(prefix, &src.norm(p)));
            }
        }
        "use_wildcard" => {
            let inner = named_children(node).into_iter().next().map(|p| src.norm(p)).unwrap_or_default();
            out.push(join(prefix, &inner));
        }
        _ => out.push(join(prefix, &src.norm(node))),
    }
}

/// `//!` comments at the top of a file.
fn inner_doc(src: Src, root: Node) -> Option<String> {
    let mut lines = Vec::new();
    let mut cursor = root.walk();
    for c in root.children(&mut cursor) {
        match c.kind() {
            "line_comment" => match src.text(c).strip_prefix("//!") {
                Some(rest) => lines.push(rest.trim().to_owned()),
                None => break,
            },
            "inner_attribute_item" => {}
            _ => break,
        }
    }
    let text = lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths_follow_rust() {
        assert_eq!(module_path("src/main.rs"), "crate");
        assert_eq!(module_path("src/orders/mod.rs"), "crate::orders");
        assert_eq!(module_path("src/orders/service.rs"), "crate::orders::service");
        assert_eq!(module_path("crates/nodex-ir/src/lib.rs"), "nodex_ir");
        assert_eq!(module_path("crates/nodex-ir/src/semantic.rs"), "nodex_ir::semantic");
        assert_eq!(module_path("crates/nodex-core/tests/operations.rs"), "nodex_core::tests::operations");
        assert_eq!(module_path("build.rs"), "crate::build");
    }
}
