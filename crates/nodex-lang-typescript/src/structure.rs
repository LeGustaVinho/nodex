//! Declarations → structure symbols, imports and type references.
//!
//! Every file is a `Module` (`ns:<path without extension>`). Declarations are named
//! `<module path>.<Name>`, members `<module path>.<Type>.<member>`.

use std::collections::HashMap;

use nodex_ir::{Language, Span, Symbol, SymbolId, SymbolKind, TypeRef, TypeRelation};
use tree_sitter::Node;

use crate::frameworks::{self, Frameworks};
use crate::syntax::{
    Src, bare_type_name, children, children_of_kind, decorator_flavor, decorators, first_child_of_kind, is_function,
    is_function_value, jsdoc, modifiers, named_children, parameters, string_value, type_names, type_text,
};

/// What kind of body a [`BodyJob`] lowers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Function,
    Method,
    Constructor,
    Getter,
    Setter,
    /// `static { … }` inside a class: runs once when the class is defined.
    StaticBlock,
    /// Statements at the top of a module: run when the module is first imported.
    TopLevel,
    /// A named function declared inside another function.
    LocalFunction,
}

/// A function-like body waiting to be lowered into a flow graph.
pub struct BodyJob<'t> {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: BodyKind,
    /// The function node (or the program for top-level code).
    pub decl: Node<'t>,
    /// `statement_block`, an expression (arrow functions), or the program.
    pub body: Node<'t>,
    /// Top-level statements (only for [`BodyKind::TopLevel`]).
    pub stmts: Vec<Node<'t>>,
    pub signature: String,
    pub params: Vec<(String, Option<String>)>,
    pub returns_void: bool,
    pub flavors: Vec<String>,
    /// Set when a framework calls this body (lifecycle, component render, registered callback):
    /// the label of its `EventEntry` ("React renders <App>").
    pub event: Option<String>,
}

pub struct Collected<'t> {
    pub symbols: Vec<Symbol>,
    /// Import targets: module paths, or `pkg:<specifier>` for packages and aliases.
    pub imports: Vec<String>,
    pub type_refs: Vec<TypeRef>,
    pub bodies: Vec<BodyJob<'t>>,
    /// Start byte of a function node with its own Blueprint → its symbol.
    pub function_ids: HashMap<usize, SymbolId>,
}

pub struct Collector<'a, 't> {
    src: Src<'a>,
    modpath: String,
    module_id: SymbolId,
    is_js: bool,
    is_dts: bool,
    symbols: Vec<Symbol>,
    type_refs: Vec<TypeRef>,
    bodies: Vec<BodyJob<'t>>,
    imports: Vec<String>,
    function_ids: HashMap<usize, SymbolId>,
    /// Functions with overload signatures (`function f(a: string): void;`).
    overloads: Vec<(SymbolId, Span)>,
    /// Name for the next anonymous function (assigned functions).
    pending_name: Option<String>,
    /// `EventEntry` label for the next function (framework callbacks and lifecycles).
    pending_event: Option<String>,
    /// Frameworks the file uses.
    fw: Frameworks,
    /// The file is the `<script setup>` of a Vue component: its top level is the setup.
    vue_setup: bool,
    /// Functions a template calls on an event (`@click="add"`, `(click)="choose(hero)"`).
    template_handlers: Vec<(String, String)>,
    /// Members being visited belong to an Angular class (constructor parameters are injected).
    in_angular_class: bool,
}

/// Which part of a Vue component's options an object literal is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VueSection {
    None,
    Root,
    Methods,
    Computed,
    Watch,
}

/// Extensions an import may carry that name code (stripped to get the module path).
const CODE_EXTENSIONS: &[&str] = &[".tsx", ".ts", ".mts", ".cts", ".jsx", ".js", ".mjs", ".cjs", ".vue"];

/// The module path of a project file: `src/app/main.ts` → `src/app/main`; `x.d.ts` → `x`.
pub fn module_path(rel: &str) -> String {
    for ext in [".d.ts", ".d.mts", ".d.cts"] {
        if let Some(stem) = rel.strip_suffix(ext) {
            return stem.to_owned();
        }
    }
    match rel.rfind('.') {
        Some(dot) if !rel[dot..].contains('/') => rel[..dot].to_owned(),
        _ => rel.to_owned(),
    }
}

/// Resolves an import specifier written in `from` (a module path) to an import target.
pub fn import_target(from: &str, spec: &str) -> Option<String> {
    if spec.is_empty() {
        return None;
    }
    if !(spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == "..") {
        return Some(format!("pkg:{spec}"));
    }
    let mut spec = spec.to_owned();
    if let Some(ext) = CODE_EXTENSIONS.iter().find(|e| spec.ends_with(*e)) {
        spec.truncate(spec.len() - ext.len());
    } else if let Some(dot) = spec.rfind('.')
        && !spec[dot..].contains('/')
        && dot > 0
        && !spec[..dot].ends_with('.')
    {
        // `./styles.css`, `./data.json`: not code.
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop(); // the file itself
    for seg in spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    if spec.ends_with('/') || spec == "." || spec == ".." {
        parts.push("index");
    }
    Some(parts.join("/"))
}

impl<'a, 't> Collector<'a, 't> {
    pub fn new(src: Src<'a>, is_js: bool, is_dts: bool, fw: Frameworks, vue_setup: bool, template_handlers: Vec<(String, String)>) -> Self {
        let modpath = module_path(src.file);
        Self {
            src,
            module_id: format!("ns:{modpath}"),
            modpath,
            is_js,
            is_dts,
            symbols: Vec::new(),
            type_refs: Vec::new(),
            bodies: Vec::new(),
            imports: Vec::new(),
            function_ids: HashMap::new(),
            overloads: Vec::new(),
            pending_name: None,
            pending_event: None,
            fw,
            vue_setup,
            template_handlers,
            in_angular_class: false,
        }
    }

    pub fn finish(mut self) -> Collected<'t> {
        for (id, span) in std::mem::take(&mut self.overloads) {
            if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                if !sym.flavors.iter().any(|f| f == "overloaded") {
                    sym.flavors.push("overloaded".into());
                }
                sym.origins.push(span);
            }
        }
        Collected {
            symbols: self.symbols,
            imports: self.imports,
            type_refs: self.type_refs,
            bodies: self.bodies,
            function_ids: self.function_ids,
        }
    }

    pub fn program(&mut self, root: Node<'t>) {
        let src = self.src;
        let file_name = src.file.rsplit('/').next().unwrap_or(src.file).to_owned();
        let mut module_flavors = Vec::new();
        if self.is_js {
            module_flavors.push("javascript".to_owned());
        }
        if self.is_dts {
            module_flavors.push("declarations".to_owned());
        }
        if src.file.ends_with(".vue") {
            module_flavors.push("vue:component".to_owned());
        }
        self.symbols.push(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: self.module_id.clone(),
            kind: SymbolKind::Module,
            name: file_name,
            qualified_name: self.modpath.clone(),
            parent: None,
            language: Language::Typescript,
            origins: vec![src.span(root)],
            flavors: module_flavors,
            signature: None,
            type_text: None,
            has_flow: false,
            external: false,
            doc: None,
        });

        let mut commonjs = false;
        self.scan_imports(root, &mut commonjs);
        let type_imports = named_children(root).into_iter().any(|node| {
            node.kind() == "import_statement" && (children(node).iter().any(|part| part.kind() == "type") ||
                first_child_of_kind(node, "import_clause").and_then(|clause| first_child_of_kind(clause, "named_imports"))
                    .is_some_and(|imports| named_children(imports).iter().any(|part| {
                        part.kind() == "import_specifier" && children(*part).iter().any(|token| token.kind() == "type")
                    })))
        });

        let mut top = Vec::new();
        let mut effectful = false;
        let mut reexports = 0;
        let mut others = 0;
        let module_id = self.module_id.clone();
        let modpath = self.modpath.clone();
        for child in named_children(root) {
            match child.kind() {
                "import_statement" | "hash_bang_line" | "empty_statement" => {}
                "export_statement" if child.child_by_field_name("source").is_some() => reexports += 1,
                _ => {
                    others += 1;
                    self.top_item(child, &module_id, &modpath, &mut top, &mut effectful);
                }
            }
        }
        let module = &mut self.symbols[0];
        if reexports > 0 && others == 0 {
            module.flavors.push("barrel".into());
        }
        if commonjs {
            module.flavors.push("commonjs".into());
        }
        if type_imports {
            module.flavors.push("type-import".into());
        }
        if self.vue_setup && !top.is_empty() {
            effectful = true;
        }
        if effectful && !self.is_dts {
            let id = format!("{}.(module)", self.modpath);
            self.symbols.push(Symbol {
                rust_contract: None,
                syntax_sites: None,
                type_info: None,
                value_info: None,
                id: id.clone(),
                kind: SymbolKind::Function,
                name: "Module top-level".into(),
                qualified_name: id.clone(),
                parent: Some(self.module_id.clone()),
                language: Language::Typescript,
                origins: vec![src.span(root)],
                flavors: vec!["top-level".into()],
                signature: Some("top-level statements".into()),
                type_text: None,
                has_flow: false,
                external: false,
                doc: None,
            });
            for s in &top {
                self.local_functions(*s, &id);
            }
            let (name, mut flavors, event) = if self.vue_setup {
                (
                    "setup".to_owned(),
                    vec!["top-level".to_owned(), "vue:setup".to_owned()],
                    Some("Vue runs this setup once for every instance of the component".to_owned()),
                )
            } else {
                ("Module top-level".to_owned(), vec!["top-level".to_owned()], None)
            };
            if type_imports {
                flavors.push("type-import".into());
            }
            self.bodies.push(BodyJob {
                symbol: id,
                name,
                kind: BodyKind::TopLevel,
                decl: root,
                body: root,
                stmts: top,
                signature: "top-level statements".into(),
                params: Vec::new(),
                returns_void: true,
                flavors,
                event,
            });
        }
    }

    /// Records `import … from`, `export … from`, `require("…")` and `import("…")` targets.
    fn scan_imports(&mut self, node: Node, commonjs: &mut bool) {
        let src = self.src;
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            let spec = match n.kind() {
                "import_statement" | "export_statement" | "import_require_clause" => {
                    n.child_by_field_name("source").map(|s| string_value(src, s))
                }
                "call_expression" => {
                    let func = n.child_by_field_name("function");
                    let is_require = func.is_some_and(|f| f.kind() == "identifier" && src.text(f) == "require");
                    let is_import = func.is_some_and(|f| f.kind() == "import");
                    if is_require {
                        *commonjs = true;
                    }
                    if is_require || is_import {
                        n.child_by_field_name("arguments")
                            .and_then(|a| named_children(a).into_iter().next())
                            .filter(|a| a.kind() == "string")
                            .map(|a| string_value(src, a))
                    } else {
                        None
                    }
                }
                "member_expression" => {
                    let t = src.text(n);
                    if t == "module.exports" || t.starts_with("exports.") {
                        *commonjs = true;
                    }
                    None
                }
                _ => None,
            };
            if let Some(target) = spec.and_then(|s| import_target(&self.modpath, &s))
                && !self.imports.contains(&target)
            {
                self.imports.push(target);
            }
            stack.extend(named_children(n));
        }
    }

    /// A statement at module (or namespace) level.
    fn top_item(&mut self, child: Node<'t>, parent: &str, qual: &str, top: &mut Vec<Node<'t>>, effectful: &mut bool) {
        let src = self.src;
        match child.kind() {
            "import_statement" | "import_alias" | "empty_statement" | "hash_bang_line" => {}
            "function_signature" => self.declaration(child, parent, qual, Vec::new(), top, effectful),
            "export_statement" => {
                if child.child_by_field_name("source").is_some() {
                    return;
                }
                let mut flavors = vec!["export".to_owned()];
                if children(child).iter().any(|c| !c.is_named() && src.text(*c) == "default") {
                    flavors.push("default-export".into());
                }
                // `@sealed export class X`: decorators written before `export`.
                flavors.extend(decorators(src, child));
                if let Some(decl) = child.child_by_field_name("declaration") {
                    self.declaration(decl, parent, qual, flavors, top, effectful);
                } else if let Some(value) = child.child_by_field_name("value") {
                    if is_function_value(value.kind()) {
                        self.function(value, None, child, parent, qual, flavors, BodyKind::Function);
                    } else if value.kind() == "class" {
                        self.class(value, parent, qual, flavors);
                    } else {
                        let options = component_options(src, value);
                        if let Some(obj) = options.or_else(|| object_literal(value)).filter(|o| has_function_members(*o, 0)) {
                            flavors.push("object".into());
                            let section = if self.fw.vue { VueSection::Root } else { VueSection::None };
                            if section == VueSection::Root {
                                flavors.push("vue:component".into());
                            }
                            let id = self.scoped_member(child, parent, qual, "default", flavors);
                            self.object_members(obj, &id, 0, section);
                        }
                        top.push(child);
                    }
                }
            }
            "expression_statement" => {
                let inner = named_children(child).into_iter().next();
                match inner.map(|i| i.kind()) {
                    Some("internal_module" | "module") => self.namespace(inner.unwrap_or(child), parent, qual, Vec::new()),
                    // Directives (`"use strict";`) do nothing visible.
                    Some("string") => {}
                    // `Counter.prototype.increment = function () { … }`, `exports.run = () => { … }`:
                    // a named function with its own Blueprint, still shown where it is assigned.
                    Some("assignment_expression") if self.assigned_function(inner.unwrap_or(child), parent, qual) => {
                        top.push(child);
                        *effectful = true;
                    }
                    Some("assignment_expression") if self.assigned_object(inner.unwrap_or(child), parent, qual) => {
                        top.push(child);
                        *effectful = true;
                    }
                    _ if inner.and_then(iife).is_some() => {
                        // `(async () => { … })()`: scripts often keep their whole logic here.
                        if let Some(func) = inner.and_then(iife) {
                            let n = self.symbols.iter().filter(|s| s.name.starts_with("(immediately")).count();
                            self.pending_name = Some(if n == 0 { "(immediately run)".to_owned() } else { format!("(immediately run {})", n + 1) });
                            self.function(func, None, child, parent, qual, vec!["iife".into()], BodyKind::Function);
                            self.pending_name = None;
                        }
                        top.push(child);
                        *effectful = true;
                    }
                    _ => {
                        top.push(child);
                        *effectful = true;
                    }
                }
            }
            "lexical_declaration" | "variable_declaration" | "function_declaration" | "generator_function_declaration"
            | "class_declaration" | "abstract_class_declaration" | "interface_declaration" | "type_alias_declaration"
            | "enum_declaration" | "internal_module" | "module" | "ambient_declaration" => {
                self.declaration(child, parent, qual, Vec::new(), top, effectful)
            }
            _ => {
                top.push(child);
                *effectful = true;
            }
        }
    }

    /// Registers `a.b = function …` (block body) as a function named `a.b` (`prototype` dropped).
    /// Returns false when the assignment is something else.
    fn assigned_function(&mut self, assign: Node<'t>, parent: &str, qual: &str) -> bool {
        let src = self.src;
        let (Some(left), Some(right)) = (assign.child_by_field_name("left"), assign.child_by_field_name("right").map(crate::expr::unparen)) else {
            return false;
        };
        if left.kind() != "member_expression"
            || !is_function_value(right.kind())
            || right.child_by_field_name("body").is_none_or(|b| b.kind() != "statement_block")
        {
            return false;
        }
        let name = src.norm(left).replace(".prototype.", ".");
        let name = name.strip_prefix("module.exports.").or_else(|| name.strip_prefix("exports.")).unwrap_or(&name).to_owned();
        let (owner, short) = match name.rsplit_once('.') {
            Some((o, m)) if self.symbols.iter().any(|s| s.id == format!("{qual}.{o}")) => (format!("{qual}.{o}"), m.to_owned()),
            _ => (parent.to_owned(), name.clone()),
        };
        let owner_qual = if owner == parent { qual.to_owned() } else { owner.clone() };
        let mut flavors = Vec::new();
        if src.text(left).contains(".prototype.") {
            flavors.push("prototype".to_owned());
        }
        if src.text(left).starts_with("exports.") || src.text(left).starts_with("module.exports") {
            flavors.push("commonjs".to_owned());
            flavors.push("export".to_owned());
        }
        self.pending_name = Some(short);
        self.function(right, None, assign, &owner, &owner_qual, flavors, BodyKind::Function);
        self.pending_name = None;
        true
    }

    /// `module.exports = { … }` / `exports.api = { … }` holding functions: a field for the
    /// object with a Blueprint per function member.
    fn assigned_object(&mut self, assign: Node<'t>, parent: &str, qual: &str) -> bool {
        let src = self.src;
        let (Some(left), Some(right)) = (assign.child_by_field_name("left"), assign.child_by_field_name("right")) else {
            return false;
        };
        let Some(obj) = object_literal(right).filter(|o| has_function_members(*o, 0)) else { return false };
        if left.kind() != "member_expression" {
            return false;
        }
        let target = src.norm(left);
        let name = target.strip_prefix("module.").unwrap_or(&target).to_owned();
        let mut flavors = vec!["object".to_owned()];
        if target.starts_with("module.exports") || target.starts_with("exports.") {
            flavors.extend(["commonjs".to_owned(), "export".to_owned()]);
        }
        let id = self.scoped_member(assign, parent, qual, &name, flavors);
        self.object_members(obj, &id, 0, VueSection::None);
        true
    }

    /// A field `<qual>.<name>` contained by `parent` (module-level holders of object literals).
    fn scoped_member(&mut self, node: Node, parent: &str, qual: &str, name: &str, flavors: Vec<String>) -> SymbolId {
        let src = self.src;
        let id = format!("{qual}.{name}");
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Field,
            name: name.to_owned(),
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(node)],
            flavors,
            signature: Some(nodex_lang_api::kit::truncate(&src.norm(node), 80)),
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, node),
        });
        id
    }

    fn declaration(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>, top: &mut Vec<Node<'t>>, effectful: &mut bool) {
        let src = self.src;
        match decl.kind() {
            "function_declaration" | "generator_function_declaration" => {
                self.function(decl, decl.child_by_field_name("name"), decl, parent, qual, flavors, BodyKind::Function)
            }
            "class_declaration" | "abstract_class_declaration" | "class" => self.class(decl, parent, qual, flavors),
            "interface_declaration" => self.interface(decl, parent, qual, flavors),
            "type_alias_declaration" => self.type_alias(decl, parent, qual, flavors),
            "enum_declaration" => self.enumeration(decl, parent, qual, flavors),
            "lexical_declaration" | "variable_declaration" => self.variables(decl, parent, qual, flavors, top),
            // Overload signatures merge into the implementation.
            "function_signature" => {
                if let Some(name) = decl.child_by_field_name("name") {
                    self.overloads.push((format!("{qual}.{}", src.norm(name)), src.span(decl)));
                }
            }
            "internal_module" | "module" => self.namespace(decl, parent, qual, flavors),
            "ambient_declaration" => {
                let mut flavors = flavors;
                flavors.push("declare".into());
                for inner in named_children(decl) {
                    if inner.kind() == "statement_block" {
                        // `declare global { … }` adds declarations to the global scope.
                        if src.text(decl).trim_start_matches("declare").trim_start().starts_with("global") {
                            self.global_augmentation(decl, inner, parent, qual);
                        }
                        continue;
                    }
                    self.declaration(inner, parent, qual, flavors.clone(), &mut Vec::new(), effectful);
                }
            }
            "expression_statement" => {
                if let Some(inner) = named_children(decl).into_iter().next().filter(|i| matches!(i.kind(), "internal_module" | "module")) {
                    self.namespace(inner, parent, qual, flavors);
                }
            }
            _ => {
                top.push(decl);
                *effectful = true;
            }
        }
    }

    /// `const a = 1, f = () => …`: functions become Blueprints, values become fields.
    fn variables(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>, top: &mut Vec<Node<'t>>) {
        let src = self.src;
        let keyword = children(decl).into_iter().find(|c| !c.is_named()).map(|c| src.text(c).to_owned()).unwrap_or_default();
        let mut has_values = false;
        for d in children_of_kind(decl, "variable_declarator") {
            let name = d.child_by_field_name("name");
            let value = d.child_by_field_name("value").map(crate::expr::unparen);
            if let (Some(n), Some(v)) = (name, value)
                && n.kind() == "identifier"
                && is_function_value(v.kind())
            {
                let mut f = flavors.clone();
                f.push(keyword.clone());
                self.function(v, Some(n), d, parent, qual, f, BodyKind::Function);
                continue;
            }
            // `const Row = memo(function Row() { … })`, `const Input = forwardRef((props, ref) => …)`.
            if let (Some(n), Some(v)) = (name, value)
                && n.kind() == "identifier"
                && let Some((inner, flavor)) = wrapped_function(src, v)
            {
                let mut f = flavors.clone();
                f.push(keyword.clone());
                f.push(flavor.into());
                self.function(inner, Some(n), d, parent, qual, f, BodyKind::Function);
                continue;
            }
            // `const Foo = class { … }`: a class named after the constant.
            if let (Some(n), Some(v)) = (name, value)
                && n.kind() == "identifier"
                && v.kind() == "class"
            {
                self.pending_name = Some(src.norm(n));
                let mut f = flavors.clone();
                f.push("class-expression".into());
                self.class(v, parent, qual, f);
                self.pending_name = None;
                continue;
            }
            has_values = true;
            let Some(n) = name else { continue };
            let mut names = Vec::new();
            crate::syntax::pattern_names(src, n, &mut names);
            let ty = d.child_by_field_name("type").map(|t| type_text(src, t));
            for field in names {
                let id = format!("{qual}.{field}");
                let mut f = flavors.clone();
                f.push(keyword.clone());
                if value.is_some_and(|v| v.kind() == "call_expression" && v.child_by_field_name("function").is_some_and(|f| src.text(f) == "require")) {
                    f.push("commonjs".into());
                }
                let object = value.and_then(object_literal).filter(|o| has_function_members(*o, 0));
                if object.is_some() {
                    f.push("object".into());
                }
                self.push_or_merge(Symbol {
                    rust_contract: None,
                    syntax_sites: None,
                    type_info: None,
                    value_info: None,
                    id: id.clone(),
                    kind: SymbolKind::Field,
                    name: field.clone(),
                    qualified_name: id.clone(),
                    parent: Some(parent.to_owned()),
                    language: Language::Typescript,
                    origins: vec![src.span(d)],
                    flavors: f,
                    signature: Some(format!("{keyword} {}", src.header(d, value))
                        .trim_end_matches('=')
                        .trim()
                        .to_owned()),
                    type_text: ty.clone(),
                    has_flow: false,
                    external: false,
                    doc: jsdoc(src, decl),
                });
                if let Some(t) = d.child_by_field_name("type") {
                    self.refs(&id, t);
                }
                if let Some(obj) = value.and_then(object_literal).filter(|o| has_function_members(*o, 0)) {
                    self.object_members(obj, &id, 0, VueSection::None);
                }
            }
        }
        if has_values {
            top.push(decl);
        }
    }

    /// Functions written as members of an object literal (`const api = { get() { … } }`,
    /// `module.exports = { run: () => … }`, `export default { methods: { … } }`): each gets a
    /// Blueprint under the object's symbol. Nested objects that hold functions get a field.
    fn object_members(&mut self, obj: Node<'t>, owner: &str, depth: usize, section: VueSection) {
        let src = self.src;
        for m in named_children(obj) {
            match m.kind() {
                "method_definition" => {
                    let key = m.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                    let id = format!("{owner}.{key}");
                    let flavors = self.vue_member(section, &key);
                    self.function(m, m.child_by_field_name("name"), m, owner, owner, flavors, BodyKind::Method);
                    if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                        sym.kind = SymbolKind::Method;
                    }
                }
                "pair" => {
                    let (Some(key), Some(value)) = (m.child_by_field_name("key"), m.child_by_field_name("value").map(crate::expr::unparen)) else {
                        continue;
                    };
                    let name = if key.kind() == "string" { string_value(src, key) } else { src.norm(key) };
                    if is_function_value(value.kind()) {
                        let id = format!("{owner}.{name}");
                        let flavors = self.vue_member(section, &name);
                        self.pending_name = Some(name);
                        self.function(value, None, m, owner, owner, flavors, BodyKind::Method);
                        self.pending_name = None;
                        if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                            sym.kind = SymbolKind::Method;
                        }
                    } else if let Some(inner) = object_literal(value).filter(|o| depth < 2 && has_function_members(*o, depth + 1)) {
                        let inner_section = match (section, name.as_str()) {
                            (VueSection::Root, "methods") => VueSection::Methods,
                            (VueSection::Root, "computed") => VueSection::Computed,
                            (VueSection::Root, "watch") => VueSection::Watch,
                            _ => VueSection::None,
                        };
                        let id = self.member(m, owner, &name, SymbolKind::Field, vec!["object".into()], None);
                        self.object_members(inner, &id, depth + 1, inner_section);
                    }
                }
                _ => {}
            }
        }
    }

    /// The `EventEntry` label of a function a template calls on an event.
    fn template_event(&self, name: &str) -> Option<String> {
        let events: Vec<&str> = self.template_handlers.iter().filter(|(f, _)| f == name).map(|(_, e)| e.as_str()).collect();
        if events.is_empty() {
            return None;
        }
        let framework = if self.fw.angular { "Angular" } else { "Vue" };
        Some(format!("{framework} calls this when the template raises {}", events.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(" or ")))
    }

    /// Flavors (and the pending `EventEntry` label) of a member of a Vue component's options.
    fn vue_member(&mut self, section: VueSection, key: &str) -> Vec<String> {
        let mut flavors = vec!["object-member".to_owned()];
        match section {
            VueSection::None => {}
            VueSection::Root => {
                if let Some(event) = frameworks::vue_option(key) {
                    flavors.push("vue:lifecycle".into());
                    self.pending_event = Some(event.to_owned());
                }
            }
            VueSection::Methods => {
                flavors.push("vue:method".into());
                self.pending_event = self.template_event(key);
            }
            VueSection::Computed => {
                flavors.push("vue:computed".into());
                self.pending_event = Some(format!("Vue runs this when {key} is read and a value it depends on has changed"));
            }
            VueSection::Watch => {
                flavors.push("vue:watch".into());
                self.pending_event = Some(format!("Vue runs this when {key} changes"));
            }
        }
        flavors
    }

    /// `declare global { interface Window { … } }`: a `global` namespace in the module.
    fn global_augmentation(&mut self, decl: Node<'t>, body: Node<'t>, parent: &str, qual: &str) {
        let src = self.src;
        let id = format!("{qual}.global");
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Namespace,
            name: "global".into(),
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors: vec!["declare".into(), "global-augmentation".into()],
            signature: Some("declare global".into()),
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, decl),
        });
        let mut ignored = false;
        for child in named_children(body) {
            self.top_item(child, &id, &id, &mut Vec::new(), &mut ignored);
        }
    }

    /// A function, arrow function or function expression with a name.
    #[allow(clippy::too_many_arguments)]
    fn function(&mut self, func: Node<'t>, name: Option<Node>, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>, kind: BodyKind) {
        let src = self.src;
        let name = name.map(|n| src.norm(n)).or_else(|| self.pending_name.clone()).unwrap_or_else(|| "default".into());
        let mut event = self.pending_event.take();
        if event.is_none() && self.vue_setup && kind == BodyKind::Function {
            event = self.template_event(&name);
        }
        let mut id = format!("{qual}.{name}");
        if kind == BodyKind::LocalFunction {
            let mut i = 2;
            while self.symbols.iter().any(|s| s.id == id) {
                id = format!("{qual}.{name}~{i}");
                i += 1;
            }
        }
        let mut flavors = flavors;
        for m in modifiers(src, func) {
            if !flavors.contains(&m) {
                flavors.push(m);
            }
        }
        if func.kind() == "arrow_function" {
            flavors.push("arrow".into());
        }
        if matches!(func.kind(), "generator_function" | "generator_function_declaration") && !flavors.iter().any(|f| f == "generator") {
            flavors.push("generator".into());
        }
        if kind == BodyKind::LocalFunction {
            flavors.push("local-function".into());
        }
        if self.is_dts {
            flavors.push("declare".into());
        }
        let body = func.child_by_field_name("body");
        if self.fw.react && kind != BodyKind::Method {
            if frameworks::is_component_name(&name) && body.is_some_and(contains_jsx) {
                flavors.push("react:component".into());
                event.get_or_insert_with(|| format!("React renders <{name}> (again whenever its props or state change)"));
            } else if frameworks::is_hook_name(&name) {
                flavors.push("react:hook".into());
            }
        }
        flavors.dedup();
        let returns = func.child_by_field_name("return_type").map(|t| type_text(src, t));
        let header = src.header(decl, body);
        let signature = match decl.kind() {
            "variable_declarator" => {
                let kw = decl.parent().and_then(|p| children(p).into_iter().find(|c| !c.is_named())).map(|c| src.text(c)).unwrap_or("const");
                format!("{kw} {header}")
            }
            _ => header,
        };
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Function,
            name: name.clone(),
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors: flavors.clone(),
            signature: Some(signature.trim_end_matches(';').trim().to_owned()),
            type_text: returns.clone(),
            has_flow: false,
            external: false,
            doc: jsdoc(src, if decl.kind() == "variable_declarator" { decl.parent().unwrap_or(decl) } else { decl }),
        });
        self.param_refs(&id, func);
        if let Some(body) = body.filter(|_| !self.is_dts) {
            self.function_ids.insert(func.start_byte(), id.clone());
            self.local_functions(body, &id);
            self.bodies.push(BodyJob {
                symbol: id,
                name,
                kind,
                decl: func,
                body,
                stmts: Vec::new(),
                signature,
                params: parameters(src, func),
                returns_void: returns.as_deref().is_some_and(is_void),
                flavors,
                event,
            });
        }
    }

    /// Named functions declared inside a body: `function helper() {}` and
    /// `const onClick = () => { … }`. Each gets its own Blueprint.
    fn local_functions(&mut self, body: Node<'t>, owner: &str) {
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            for c in named_children(n) {
                match c.kind() {
                    "function_declaration" | "generator_function_declaration" => {
                        if !self.function_ids.contains_key(&c.start_byte()) {
                            self.function(c, c.child_by_field_name("name"), c, owner, owner, Vec::new(), BodyKind::LocalFunction);
                        }
                    }
                    "call_expression" => {
                        // `useEffect(() => { … })`, `app.get('/x', (req, res) => { … })`: the
                        // framework calls the callback; it gets a Blueprint of its own.
                        if let Some(reg) = frameworks::registration(self.src, self.fw, c)
                            && reg.callback.child_by_field_name("body").is_some_and(|b| b.kind() == "statement_block")
                            && !self.function_ids.contains_key(&reg.callback.start_byte())
                        {
                            self.pending_name = Some(reg.name.clone());
                            self.pending_event = Some(reg.event.clone());
                            self.function(reg.callback, None, reg.callback, owner, owner, vec![reg.flavor.to_owned()], BodyKind::LocalFunction);
                            self.pending_name = None;
                            self.pending_event = None;
                        }
                        // Any other callback with a block body (`items.forEach(item => { … })`,
                        // `new Promise`-less `then(r => { … })`): a Blueprint `<callee> callback`.
                        let callee = c.child_by_field_name("function").map(|f| match f.kind() {
                            "member_expression" => f.child_by_field_name("property").map(|p| self.src.norm(p)).unwrap_or_default(),
                            "identifier" => self.src.norm(f),
                            _ => String::new(),
                        });
                        if let Some(args) = c.child_by_field_name("arguments").filter(|a| a.kind() == "arguments") {
                            for arg in named_children(args) {
                                if is_function_value(arg.kind())
                                    && arg.child_by_field_name("body").is_some_and(|b| {
                                        b.kind() == "statement_block" && named_children(b).iter().any(|s| s.kind() != "comment")
                                    })
                                    && !self.function_ids.contains_key(&arg.start_byte())
                                {
                                    let callee = callee.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "call".into());
                                    self.pending_name = Some(format!("{callee} callback"));
                                    self.function(arg, None, arg, owner, owner, vec!["callback".into()], BodyKind::LocalFunction);
                                    self.pending_name = None;
                                }
                            }
                        }
                        stack.push(c);
                    }
                    "variable_declarator" => {
                        let name = c.child_by_field_name("name").filter(|n| n.kind() == "identifier");
                        let value = c.child_by_field_name("value").map(crate::expr::unparen);
                        if let (Some(nm), Some(v)) = (name, value)
                            && let Some((inner, flavor)) = wrapped_function(self.src, v)
                            && inner.child_by_field_name("body").is_some_and(|b| b.kind() == "statement_block")
                        {
                            if !self.function_ids.contains_key(&inner.start_byte()) {
                                self.function(inner, Some(nm), c, owner, owner, vec![flavor.to_owned()], BodyKind::LocalFunction);
                            }
                            continue;
                        }
                        match (name, value) {
                            (Some(nm), Some(v))
                                if is_function_value(v.kind())
                                    && v.child_by_field_name("body").is_some_and(|b| b.kind() == "statement_block") =>
                            {
                                if !self.function_ids.contains_key(&v.start_byte()) {
                                    self.function(v, Some(nm), c, owner, owner, Vec::new(), BodyKind::LocalFunction);
                                }
                            }
                            _ => stack.push(c),
                        }
                    }
                    k if is_function(k) => {}
                    _ => stack.push(c),
                }
            }
        }
    }

    fn class(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>) {
        let src = self.src;
        let name = decl
            .child_by_field_name("name")
            .map(|n| src.norm(n))
            .or_else(|| self.pending_name.take())
            .unwrap_or_else(|| "default".into());
        let id = format!("{qual}.{name}");
        let body = decl.child_by_field_name("body");
        let mut flavors = flavors;
        if decl.kind() == "abstract_class_declaration" {
            flavors.push("abstract".into());
        }
        flavors.extend(decorators(src, decl));
        let angular = frameworks::angular_flavors(&flavors);
        let angular_class = !angular.is_empty();
        flavors.extend(angular);
        let was_angular = self.in_angular_class;
        self.in_angular_class = angular_class;
        let react_class = self.fw.react
            && first_child_of_kind(decl, "class_heritage")
                .and_then(|h| first_child_of_kind(h, "extends_clause"))
                .and_then(|e| named_children(e).into_iter().next())
                .is_some_and(|b| frameworks::react_class_base(&bare_type_name(&src.norm(b))));
        if react_class {
            flavors.push("react:component".into());
        }
        if self.is_dts {
            flavors.push("declare".into());
        }
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Class,
            name: name.clone(),
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors,
            signature: Some(src.header(decl, body)),
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, decl),
        });
        if let Some(heritage) = first_child_of_kind(decl, "class_heritage") {
            for clause in named_children(heritage) {
                for base in named_children(clause).into_iter().filter(|b| b.kind() != "type_arguments") {
                    let name = bare_type_name(&src.norm(base));
                    if !name.is_empty() {
                        self.type_refs.push(TypeRef { from: id.clone(), name, relation: TypeRelation::Base });
                    }
                }
            }
        }
        let Some(body) = body else {
            self.in_angular_class = was_angular;
            return;
        };
        let mut pending: Vec<String> = Vec::new();
        let mut host_event: Option<String> = None;
        for m in named_children(body) {
            match m.kind() {
                "decorator" => {
                    pending.push(decorator_flavor(src, m));
                    if let Some(e) = frameworks::host_listener(src, m) {
                        host_event = Some(e);
                    }
                }
                "method_definition" => {
                    let mname = m.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                    let mut decorators = std::mem::take(&mut pending);
                    let lifecycle = if react_class {
                        frameworks::react_lifecycle(&mname).map(|e| (e, "react:lifecycle"))
                    } else if angular_class || self.fw.angular {
                        frameworks::angular_lifecycle(&mname).map(|e| (e, "angular:lifecycle"))
                    } else {
                        None
                    };
                    self.pending_event = host_event.take().or_else(|| lifecycle.map(|(e, _)| e.to_owned()));
                    if let Some((_, flavor)) = lifecycle {
                        decorators.push(flavor.into());
                    }
                    self.method(m, &id, &name, decorators);
                    self.pending_event = None;
                }
                "method_signature" | "abstract_method_signature" => {
                    let mut f = std::mem::take(&mut pending);
                    f.extend(modifiers(src, m));
                    if m.kind() == "abstract_method_signature" && !f.iter().any(|x| x == "abstract") {
                        f.push("abstract".into());
                    }
                    let mname = m.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
                    self.member(m, &id, &mname, SymbolKind::Method, f, m.child_by_field_name("return_type"));
                    self.param_refs(&format!("{id}.{mname}"), m);
                }
                "public_field_definition" => self.field(m, &id, std::mem::take(&mut pending)),
                "class_static_block" => {
                    let sid = format!("{id}.static");
                    self.member(m, &id, "static", SymbolKind::Constructor, vec!["static".into()], None);
                    if let Some(b) = m.child_by_field_name("body") {
                        self.local_functions(b, &sid);
                        self.bodies.push(BodyJob {
                            symbol: sid,
                            name: name.clone(),
                            kind: BodyKind::StaticBlock,
                            decl: m,
                            body: b,
                            stmts: Vec::new(),
                            signature: "static { … }".into(),
                            params: Vec::new(),
                            returns_void: true,
                            flavors: vec!["static".into()],
                            event: None,
                        });
                    }
                }
                _ => pending.clear(),
            }
        }
        self.in_angular_class = was_angular;
    }

    fn method(&mut self, m: Node<'t>, class_id: &str, class_name: &str, decorators: Vec<String>) {
        let src = self.src;
        let name_node = m.child_by_field_name("name");
        let name = name_node
            .map(|n| if n.kind() == "string" { string_value(src, n) } else { src.norm(n) })
            .unwrap_or_else(|| "?".into());
        let mut flavors = modifiers(src, m);
        flavors.extend(decorators);
        let ng = frameworks::angular_flavors(&flavors);
        flavors.extend(ng);
        if name.starts_with('#') {
            flavors.push("private".into());
            flavors.push("runtime-private".into());
        }
        let event = self.pending_event.take().or_else(|| self.template_event(&name));
        let body = m.child_by_field_name("body");
        let params = parameters(src, m);
        let returns = m.child_by_field_name("return_type").map(|t| type_text(src, t));
        let is_getter = flavors.iter().any(|f| f == "getter");
        let is_setter = flavors.iter().any(|f| f == "setter");
        let (id, kind, body_kind, label) = if name == "constructor" {
            self.parameter_properties(m, class_id);
            (format!("{class_id}.constructor"), SymbolKind::Constructor, BodyKind::Constructor, class_name.to_owned())
        } else if is_getter || is_setter {
            let prop_flavors: Vec<String> = flavors.iter().filter(|f| !matches!(f.as_str(), "getter" | "setter")).cloned().collect();
            let prop_ty = if is_getter { returns.clone() } else { params.first().and_then(|(_, t)| t.clone()) };
            let prop_id = self.member(m, class_id, &name, SymbolKind::Property, prop_flavors, None);
            if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == prop_id) {
                if sym.type_text.is_none() {
                    sym.type_text = prop_ty;
                }
                let tag = if is_getter { "computed" } else { "settable" };
                if !sym.flavors.iter().any(|f| f == tag) {
                    sym.flavors.push(tag.into());
                }
            }
            let keyword = if is_getter { "get" } else { "set" };
            let acc_id = format!("{prop_id}.{keyword}");
            self.push_or_merge(Symbol {
                rust_contract: None,
                syntax_sites: None,
                type_info: None,
                value_info: None,
                id: acc_id.clone(),
                kind: SymbolKind::Method,
                name: format!("{name}.{keyword}"),
                qualified_name: acc_id.clone(),
                parent: Some(prop_id),
                language: Language::Typescript,
                origins: vec![src.span(m)],
                flavors: flavors.clone(),
                signature: Some(format!("{keyword} accessor of {name}")),
                type_text: returns.clone(),
                has_flow: false,
                external: false,
                doc: jsdoc(src, m),
            });
            let kind = if is_getter { BodyKind::Getter } else { BodyKind::Setter };
            if let Some(b) = body {
                self.function_ids.insert(m.start_byte(), acc_id.clone());
                self.local_functions(b, &acc_id);
                self.bodies.push(BodyJob {
                    symbol: acc_id,
                    name: format!("{name}.{keyword}"),
                    kind,
                    decl: m,
                    body: b,
                    stmts: Vec::new(),
                    signature: src.header(m, Some(b)),
                    params,
                    returns_void: !is_getter,
                    flavors,
                    event: None,
                });
            }
            return;
        } else {
            (format!("{class_id}.{name}"), SymbolKind::Method, BodyKind::Method, name.clone())
        };
        let signature = src.header(m, body);
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind,
            name: if kind == SymbolKind::Constructor { class_name.to_owned() } else { name.clone() },
            qualified_name: id.clone(),
            parent: Some(class_id.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(m)],
            flavors: flavors.clone(),
            signature: Some(signature.trim_end_matches(';').trim().to_owned()),
            type_text: returns.clone(),
            has_flow: false,
            external: false,
            doc: jsdoc(src, m),
        });
        self.param_refs(&id, m);
        if let Some(b) = body {
            self.function_ids.insert(m.start_byte(), id.clone());
            self.local_functions(b, &id);
            self.bodies.push(BodyJob {
                symbol: id,
                name: label,
                kind: body_kind,
                decl: m,
                body: b,
                stmts: Vec::new(),
                signature,
                params,
                returns_void: kind == SymbolKind::Constructor || returns.as_deref().is_some_and(is_void),
                flavors,
                event,
            });
        }
    }

    /// `constructor(private readonly repo: Repo)` declares the field `repo`.
    fn parameter_properties(&mut self, ctor: Node, class_id: &str) {
        let src = self.src;
        let Some(list) = ctor.child_by_field_name("parameters") else { return };
        for p in named_children(list) {
            let mods = modifiers(src, p);
            if !mods.iter().any(|m| !m.starts_with("attr:")) {
                continue;
            }
            let Some(pattern) = p.child_by_field_name("pattern").filter(|n| n.kind() == "identifier") else { continue };
            let mut flavors = mods;
            flavors.push("parameter-property".into());
            if self.in_angular_class {
                flavors.push("angular:inject".into());
            }
            let name = src.norm(pattern);
            let ty = p.child_by_field_name("type");
            let id = self.member(p, class_id, &name, SymbolKind::Field, flavors, None);
            if let Some(t) = ty {
                if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                    sym.type_text = Some(type_text(src, t));
                }
                self.refs(&id, t);
            }
        }
    }

    fn field(&mut self, m: Node<'t>, class_id: &str, decorators: Vec<String>) {
        let src = self.src;
        let name = m.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
        let mut flavors = modifiers(src, m);
        flavors.extend(decorators);
        let ng = frameworks::angular_flavors(&flavors);
        flavors.extend(ng);
        if name.starts_with('#') {
            flavors.push("private".into());
            flavors.push("runtime-private".into());
        }
        let value = m.child_by_field_name("value").map(crate::expr::unparen);
        // `private http = inject(HttpClient)`, `count = signal(0)`, `selected = ref(null)`.
        if let Some(v) = value
            && let Some((_, flavor)) = frameworks::declaration(src, self.fw, std::slice::from_ref(&name), v)
        {
            flavors.push(flavor.into());
        }
        if let Some(v) = value.filter(|v| is_function_value(v.kind())) {
            // `handle = () => { … }`: a method whose `this` is bound to the instance.
            flavors.push("bound".into());
            self.function(v, m.child_by_field_name("name"), m, class_id, class_id, flavors, BodyKind::Method);
            if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == format!("{class_id}.{name}")) {
                sym.kind = SymbolKind::Method;
            }
            return;
        }
        let ty = m.child_by_field_name("type");
        let id = self.member(m, class_id, &name, SymbolKind::Field, flavors, None);
        if let Some(t) = ty {
            if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                sym.type_text = Some(type_text(src, t));
            }
            self.refs(&id, t);
        }
    }

    fn interface(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>) {
        let src = self.src;
        let name = decl.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
        let id = format!("{qual}.{name}");
        let body = decl.child_by_field_name("body");
        let mut flavors = flavors;
        flavors.push("type-only".into());
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Interface,
            name,
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors,
            signature: Some(src.header(decl, body)),
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, decl),
        });
        if let Some(ext) = first_child_of_kind(decl, "extends_type_clause") {
            for base in named_children(ext) {
                let name = bare_type_name(&src.norm(base));
                if !name.is_empty() {
                    self.type_refs.push(TypeRef { from: id.clone(), name, relation: TypeRelation::Base });
                }
            }
        }
        if let Some(body) = body {
            self.type_members(body, &id);
        }
    }

    /// Members of an interface body or object type.
    fn type_members(&mut self, body: Node, owner: &str) {
        let src = self.src;
        for m in named_children(body) {
            let Some(name) = m.child_by_field_name("name").map(|n| src.norm(n)) else { continue };
            match m.kind() {
                "property_signature" => {
                    let flavors = modifiers(src, m);
                    let id = self.member(m, owner, &name, SymbolKind::Property, flavors, None);
                    if let Some(t) = m.child_by_field_name("type") {
                        if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                            sym.type_text = Some(type_text(src, t));
                        }
                        self.refs(&id, t);
                    }
                }
                "method_signature" => {
                    let mut flavors = modifiers(src, m);
                    flavors.push("abstract".into());
                    let id = self.member(m, owner, &name, SymbolKind::Method, flavors, m.child_by_field_name("return_type"));
                    self.param_refs(&id, m);
                }
                _ => {}
            }
        }
    }

    fn type_alias(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>) {
        let src = self.src;
        let name = decl.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
        let id = format!("{qual}.{name}");
        let value = decl.child_by_field_name("value");
        let mut flavors = flavors;
        flavors.push("type-only".into());
        if value.is_some_and(|v| v.kind() == "object_type") {
            flavors.push("object-contract".into());
        }
        let discriminant = value.and_then(|v| discriminant(src, v));
        let kind = match &discriminant {
            Some(field) => {
                flavors.push(format!("discriminated:{field}"));
                SymbolKind::Union
            }
            None => SymbolKind::TypeAlias,
        };
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind,
            name,
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors,
            signature: Some(src.norm(decl).trim_end_matches(';').to_owned()),
            type_text: value.map(|v| nodex_lang_api::kit::truncate(&src.norm(v), 120)),
            has_flow: false,
            external: false,
            doc: jsdoc(src, decl),
        });
        if let Some(v) = value {
            if v.kind() == "object_type" {
                self.type_members(v, &id);
            }
            self.refs(&id, v);
        }
    }

    fn enumeration(&mut self, decl: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>) {
        let src = self.src;
        let name = decl.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
        let id = format!("{qual}.{name}");
        let body = decl.child_by_field_name("body");
        let mut flavors = flavors;
        flavors.extend(modifiers(src, decl));
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Enum,
            name,
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(decl)],
            flavors,
            signature: Some(src.header(decl, body)),
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, decl),
        });
        let Some(body) = body else { return };
        for m in named_children(body) {
            let name_node = match m.kind() {
                "enum_assignment" => m.child_by_field_name("name"),
                "property_identifier" | "string" => Some(m),
                _ => None,
            };
            if let Some(n) = name_node {
                let member = if n.kind() == "string" { string_value(src, n) } else { src.norm(n) };
                self.member(m, &id, &member, SymbolKind::EnumMember, Vec::new(), None);
            }
        }
    }

    fn namespace(&mut self, node: Node<'t>, parent: &str, qual: &str, flavors: Vec<String>) {
        let src = self.src;
        let Some(name_node) = node.child_by_field_name("name") else { return };
        let mut flavors = flavors;
        if name_node.kind() == "string" {
            // `declare module "express" { … }`: adds declarations to another module.
            flavors.push("module-augmentation".into());
        }
        let name = src.norm(name_node);
        let id = format!("{qual}.{name}");
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind: SymbolKind::Namespace,
            name: name.clone(),
            qualified_name: id.clone(),
            parent: Some(parent.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(node)],
            flavors,
            signature: None,
            type_text: None,
            has_flow: false,
            external: false,
            doc: jsdoc(src, node),
        });
        if let Some(body) = node.child_by_field_name("body") {
            let mut ignored = false;
            for child in named_children(body) {
                self.top_item(child, &id, &id, &mut Vec::new(), &mut ignored);
            }
        }
    }

    /// Adds a member symbol `<owner>.<name>` (merging repeated declarations such as overloads).
    fn member(&mut self, node: Node, owner: &str, name: &str, kind: SymbolKind, flavors: Vec<String>, returns: Option<Node>) -> SymbolId {
        let src = self.src;
        let id = format!("{owner}.{name}");
        let body = node.child_by_field_name("body").or_else(|| node.child_by_field_name("value"));
        self.push_or_merge(Symbol {
            rust_contract: None,
            syntax_sites: None,
            type_info: None,
            value_info: None,
            id: id.clone(),
            kind,
            name: name.to_owned(),
            qualified_name: id.clone(),
            parent: Some(owner.to_owned()),
            language: Language::Typescript,
            origins: vec![src.span(node)],
            flavors,
            signature: Some(src.header(node, body).trim_end_matches(['=', ';']).trim().to_owned()),
            type_text: returns.map(|r| type_text(src, r)),
            has_flow: false,
            external: false,
            doc: jsdoc(src, node),
        });
        if let Some(r) = returns {
            self.refs(&id, r);
        }
        id
    }

    fn param_refs(&mut self, from: &str, func: Node) {
        if let Some(list) = func.child_by_field_name("parameters") {
            for p in named_children(list) {
                if let Some(t) = p.child_by_field_name("type") {
                    self.refs(from, t);
                }
            }
        }
        if let Some(r) = func.child_by_field_name("return_type") {
            self.refs(from, r);
        }
    }

    fn refs(&mut self, from: &str, ty: Node) {
        let mut names = Vec::new();
        type_names(self.src, ty, &mut names);
        for name in names {
            self.type_refs.push(TypeRef { from: from.to_owned(), name, relation: TypeRelation::References });
        }
    }

    fn push_or_merge(&mut self, symbol: Symbol) {
        if let Some(existing) = self.symbols.iter_mut().find(|s| s.id == symbol.id) {
            // Overloads and declaration merging: one symbol with several origins.
            existing.origins.extend(symbol.origins);
            for f in symbol.flavors {
                if !existing.flavors.contains(&f) {
                    existing.flavors.push(f);
                }
            }
            if existing.origins.len() > 1 && existing.kind == SymbolKind::Function && !existing.flavors.iter().any(|f| f == "overloaded") {
                existing.flavors.push("overloaded".into());
            }
            if existing.doc.is_none() {
                existing.doc = symbol.doc;
            }
        } else {
            self.symbols.push(symbol);
        }
    }
}

/// The function of an immediately invoked function expression statement:
/// `(function () { … })()`, `(async () => { … })()`, `void (async () => { … })()`.
fn iife(expr: Node) -> Option<Node> {
    let mut e = crate::expr::unparen(expr);
    if e.kind() == "unary_expression" || e.kind() == "await_expression" {
        e = crate::expr::unparen(named_children(e).into_iter().last()?);
    }
    if e.kind() != "call_expression" {
        return None;
    }
    let callee = e.child_by_field_name("function")?;
    if callee.kind() != "parenthesized_expression" {
        return None;
    }
    let func = crate::expr::unparen(callee);
    (is_function_value(func.kind()) && func.child_by_field_name("body").is_some_and(|b| b.kind() == "statement_block")).then_some(func)
}

/// The function inside a wrapper call (`memo(…)`, `forwardRef(…)`, `useCallback(…, deps)`).
fn wrapped_function<'t>(src: Src, value: Node<'t>) -> Option<(Node<'t>, &'static str)> {
    let call = crate::expr::unparen(value);
    if call.kind() != "call_expression" {
        return None;
    }
    let flavor = frameworks::function_wrapper(&call.child_by_field_name("function").map(|f| src.norm(f))?)?;
    let inner = call.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next()).map(crate::expr::unparen)?;
    is_function_value(inner.kind()).then_some((inner, flavor))
}

/// The options object of `defineComponent({ … })` / `Vue.extend({ … })`.
fn component_options<'t>(src: Src, value: Node<'t>) -> Option<Node<'t>> {
    let call = crate::expr::unparen(value);
    if call.kind() != "call_expression" {
        return None;
    }
    let callee = call.child_by_field_name("function").map(|f| src.norm(f))?;
    if !matches!(callee.as_str(), "defineComponent" | "Vue.extend" | "defineNuxtComponent") {
        return None;
    }
    let arg = call.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next())?;
    object_literal(arg)
}

/// True if JSX appears in `node`, outside nested functions.
fn contains_jsx(node: Node) -> bool {
    named_children(node).into_iter().any(|c| {
        matches!(c.kind(), "jsx_element" | "jsx_self_closing_element") || (!is_function(c.kind()) && contains_jsx(c))
    })
}

/// The object literal a value is (through parentheses, `as const` and `satisfies`).
fn object_literal(value: Node) -> Option<Node> {
    let v = crate::expr::unparen(value);
    match v.kind() {
        "object" => Some(v),
        "as_expression" | "satisfies_expression" => named_children(v).into_iter().next().and_then(object_literal),
        _ => None,
    }
}

/// True if an object literal (or one nested up to depth 2) has function members.
fn has_function_members(obj: Node, depth: usize) -> bool {
    named_children(obj).into_iter().any(|m| match m.kind() {
        "method_definition" => m.child_by_field_name("body").is_some(),
        "pair" => m.child_by_field_name("value").map(crate::expr::unparen).is_some_and(|v| {
            is_function_value(v.kind()) || (depth < 2 && object_literal(v).is_some_and(|o| has_function_members(o, depth + 1)))
        }),
        _ => false,
    })
}

fn is_void(ty: &str) -> bool {
    matches!(ty, "void" | "Promise<void>" | "never" | "undefined")
}

/// The discriminant property of a union of object types that all have it with a literal type:
/// `{ kind: "a" } | { kind: "b"; x: number }` → `kind`.
fn discriminant(src: Src, value: Node) -> Option<String> {
    fn members<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
        if n.kind() == "union_type" {
            for c in named_children(n) {
                members(c, out);
            }
        } else {
            out.push(n);
        }
    }
    if value.kind() != "union_type" {
        return None;
    }
    let mut parts = Vec::new();
    members(value, &mut parts);
    if parts.len() < 2 || parts.iter().any(|p| p.kind() != "object_type") {
        return None;
    }
    let literal_props = |obj: Node| -> Vec<String> {
        children_of_kind(obj, "property_signature")
            .into_iter()
            .filter(|p| {
                p.child_by_field_name("type")
                    .and_then(|t| named_children(t).into_iter().next())
                    .is_some_and(|t| t.kind() == "literal_type")
            })
            .filter_map(|p| p.child_by_field_name("name").map(|n| src.norm(n)))
            .collect()
    };
    let first = literal_props(parts[0]);
    first.into_iter().find(|name| parts[1..].iter().all(|p| literal_props(*p).contains(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths_drop_extensions() {
        assert_eq!(module_path("src/app/main.ts"), "src/app/main");
        assert_eq!(module_path("types/global.d.ts"), "types/global");
        assert_eq!(module_path("src/foo.test.tsx"), "src/foo.test");
    }

    #[test]
    fn import_targets_resolve_relative_paths() {
        assert_eq!(import_target("src/app/main", "./util").as_deref(), Some("src/app/util"));
        assert_eq!(import_target("src/app/main", "../models/order.js").as_deref(), Some("src/models/order"));
        assert_eq!(import_target("src/app/main", "..").as_deref(), Some("src/index"));
        assert_eq!(import_target("src/app/main", "./styles.css"), None);
        assert_eq!(import_target("src/app/main", "react").as_deref(), Some("pkg:react"));
        assert_eq!(import_target("src/app/main", "@/lib/x").as_deref(), Some("pkg:@/lib/x"));
    }
}
