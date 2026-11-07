//! Declarations → structure symbols, imports and type references.

use nodex_ir::{ImportRef, Language, Symbol, SymbolId, SymbolKind, TypeRef, TypeRelation};
use tree_sitter::Node;

use crate::syntax::{
    Src, attributes, bare_type_name, children, children_of_kind, contains_kind,
    first_child_of_kind, modifiers, named_children, type_names,
};
use crate::unity;

/// What kind of body a [`BodyJob`] lowers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Method,
    Constructor,
    /// Property expression body or `get` accessor: the expression is returned.
    Getter,
    /// `set`/`init`/`add`/`remove` accessor or anything returning nothing.
    Setter,
    /// Top-level statements (C# 9+ programs without `Main`).
    TopLevel,
}

/// A function-like body waiting to be lowered into a flow graph.
pub struct BodyJob<'t> {
    pub symbol: SymbolId,
    pub name: String,
    pub kind: BodyKind,
    /// The declaration node (method, constructor, accessor, property) or the compilation unit.
    pub decl: Node<'t>,
    /// `block` or `arrow_expression_clause`; for top-level programs, the compilation unit.
    pub body: Node<'t>,
    pub signature: String,
    /// `(name, type)` of each parameter.
    pub params: Vec<(String, Option<String>)>,
    pub returns_void: bool,
    pub flavors: Vec<String>,
    /// Unity lifecycle message implemented by this method (`Update`, `OnCollisionEnter`, ...).
    pub unity_message: Option<&'static str>,
    pub unity: bool,
}

pub struct Collected<'t> {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<ImportRef>,
    pub type_refs: Vec<TypeRef>,
    pub bodies: Vec<BodyJob<'t>>,
}

pub struct Collector<'a, 't> {
    src: Src<'a>,
    symbols: Vec<Symbol>,
    type_refs: Vec<TypeRef>,
    bodies: Vec<BodyJob<'t>>,
    usings: Vec<String>,
    namespaces: Vec<String>,
    unity_file: bool,
}

const GLOBAL_NS: &str = "(global)";

impl<'a, 't> Collector<'a, 't> {
    pub fn new(src: Src<'a>) -> Self {
        Self {
            src,
            symbols: Vec::new(),
            type_refs: Vec::new(),
            bodies: Vec::new(),
            usings: Vec::new(),
            namespaces: Vec::new(),
            unity_file: false,
        }
    }

    pub fn finish(mut self) -> Collected<'t> {
        if self.namespaces.is_empty() && self.symbols.iter().any(|s| s.parent.is_none()) {
            self.namespaces.push(GLOBAL_NS.to_owned());
        }
        let mut imports = Vec::new();
        for ns in &self.namespaces {
            for target in &self.usings {
                imports.push(ImportRef {
                    from_scope: ns.clone(),
                    target: target.clone(),
                });
            }
        }
        Collected {
            symbols: self.symbols,
            imports,
            type_refs: self.type_refs,
            bodies: self.bodies,
        }
    }

    pub fn compilation_unit(&mut self, root: Node<'t>) {
        // Usings first, so Unity detection is known before members are visited.
        for child in named_children(root) {
            if child.kind() == "using_directive" {
                self.using_directive(child);
            }
        }
        self.unity_file = self.usings.iter().any(|u| u == "UnityEngine" || u.starts_with("UnityEngine."));

        let mut current_ns: Option<String> = None;
        let mut top_level = Vec::new();
        for child in named_children(root) {
            match child.kind() {
                "file_scoped_namespace_declaration" => {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| self.src.norm(n))
                        .unwrap_or_default();
                    self.namespace_symbol(&name, child);
                    // Older grammars nest the members; newer ones make them siblings.
                    self.declarations(child, Some(&name), None);
                    current_ns = Some(name);
                }
                "namespace_declaration" => self.namespace(child, None),
                "global_statement" => top_level.push(child),
                _ => self.declaration(child, current_ns.as_deref(), None),
            }
        }
        if !top_level.is_empty() {
            self.top_level_program(root, current_ns.as_deref());
        }
    }

    fn using_directive(&mut self, node: Node) {
        // Skip aliases (`using X = Y;`): they have a `name` field for the alias.
        if node.child_by_field_name("name").is_some() {
            return;
        }
        let is_static = children(node).iter().any(|c| self.src.text(*c) == "static");
        if let Some(target) = named_children(node)
            .into_iter()
            .rfind(|c| matches!(c.kind(), "identifier" | "qualified_name"))
        {
            let mut name = bare_type_name(self.src.text(target));
            if is_static {
                // `using static System.Math;` imports the namespace of the type.
                if let Some((ns, _)) = name.rsplit_once('.') {
                    name = ns.to_owned();
                }
            }
            if !self.usings.contains(&name) {
                self.usings.push(name);
            }
        }
    }

    fn namespace_symbol(&mut self, name: &str, node: Node) {
        if !self.namespaces.iter().any(|n| n == name) {
            self.namespaces.push(name.to_owned());
        }
        let id = format!("ns:{name}");
        if let Some(existing) = self.symbols.iter_mut().find(|s| s.id == id) {
            existing.origins.push(self.src.span(node));
            return;
        }
        self.symbols.push(Symbol {
            id,
            kind: SymbolKind::Namespace,
            name: name.to_owned(),
            qualified_name: name.to_owned(),
            parent: None,
            language: Language::Csharp,
            origins: vec![self.src.span(node)],
            flavors: Vec::new(),
            signature: None,
            type_text: None,
            has_flow: false,
            external: false,
        });
    }

    fn namespace(&mut self, node: Node<'t>, outer: Option<&str>) {
        let short = node
            .child_by_field_name("name")
            .map(|n| self.src.norm(n))
            .unwrap_or_default();
        let name = match outer {
            Some(o) => format!("{o}.{short}"),
            None => short,
        };
        self.namespace_symbol(&name, node);
        if let Some(body) = node.child_by_field_name("body") {
            for child in named_children(body) {
                if child.kind() == "namespace_declaration" {
                    self.namespace(child, Some(&name));
                } else {
                    self.declaration(child, Some(&name), None);
                }
            }
        }
    }

    /// Visits the declarations nested directly in `node` (used for file-scoped namespaces).
    fn declarations(&mut self, node: Node<'t>, ns: Option<&str>, parent_type: Option<&TypeCtx>) {
        for child in named_children(node) {
            self.declaration(child, ns, parent_type);
        }
    }

    fn declaration(&mut self, node: Node<'t>, ns: Option<&str>, parent_type: Option<&TypeCtx>) {
        match node.kind() {
            "class_declaration" | "struct_declaration" | "interface_declaration"
            | "record_declaration" | "record_struct_declaration" | "enum_declaration"
            | "delegate_declaration" => self.type_declaration(node, ns, parent_type),
            _ => {}
        }
    }

    fn type_declaration(&mut self, node: Node<'t>, ns: Option<&str>, parent_type: Option<&TypeCtx>) {
        let src = self.src;
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let base_name = src.norm(name_node);
        let type_params = node
            .child_by_field_name("type_parameters")
            .or_else(|| first_child_of_kind(node, "type_parameter_list"));
        let arity = type_params.map(|tp| children_of_kind(tp, "type_parameter").len()).unwrap_or(0);
        let display = match type_params {
            Some(tp) => format!("{base_name}{}", src.norm(tp)),
            None => base_name.clone(),
        };
        let id_name = if arity > 0 { format!("{base_name}`{arity}") } else { base_name.clone() };

        let (qualified, parent) = match (parent_type, ns) {
            (Some(p), _) => (format!("{}.{}", p.qualified, id_name), Some(p.id.clone())),
            (None, Some(ns)) => (format!("{ns}.{id_name}"), Some(format!("ns:{ns}"))),
            (None, None) => (id_name.clone(), None),
        };

        let text_has = |kw: &str| children(node).iter().any(|c| !c.is_named() && src.text(*c) == kw);
        let kind = match node.kind() {
            "class_declaration" => SymbolKind::Class,
            "struct_declaration" => SymbolKind::Struct,
            "interface_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            "delegate_declaration" => SymbolKind::Delegate,
            _ => SymbolKind::Record,
        };

        let mut flavors = modifiers(src, node);
        flavors.extend(attributes(src, node));
        if kind == SymbolKind::Record && (text_has("struct") || node.kind() == "record_struct_declaration") {
            flavors.push("value-type".into());
        }
        if kind == SymbolKind::Struct {
            flavors.push("value-type".into());
        }
        if arity > 0 {
            flavors.push("generic".into());
        }

        let mut bases = Vec::new();
        if let Some(base_list) = first_child_of_kind(node, "base_list") {
            for b in named_children(base_list) {
                // Records may pass arguments to the base: `record B(int X) : A(X)`.
                let ty = if b.kind() == "primary_constructor_base_type" {
                    b.child_by_field_name("type").unwrap_or(b)
                } else {
                    b
                };
                if ty.kind() == "argument_list" {
                    continue;
                }
                let name = bare_type_name(src.text(ty));
                if !name.is_empty() {
                    bases.push(name);
                }
            }
        }
        let unity_base = bases.iter().find_map(|b| unity::behaviour_base(b));
        if let Some(base) = unity_base {
            flavors.push(format!("unity:{base}"));
        }
        let is_unity_type = self.unity_file && (unity_base.is_some() || !bases.is_empty());

        let signature = src.header(node, node.child_by_field_name("body").or_else(|| first_child_of_kind(node, "declaration_list")));
        self.push_or_merge(Symbol {
            id: qualified.clone(),
            kind,
            name: display,
            qualified_name: qualified.clone(),
            parent: parent.clone(),
            language: Language::Csharp,
            origins: vec![src.span(node)],
            flavors,
            signature: Some(signature),
            type_text: None,
            has_flow: false,
            external: false,
        });
        for base in bases {
            self.type_refs.push(TypeRef {
                from: qualified.clone(),
                name: base,
                relation: TypeRelation::Base,
            });
        }

        let ctx = TypeCtx {
            id: qualified.clone(),
            qualified,
            name: base_name,
            kind,
            unity: is_unity_type,
        };

        // Positional records: parameters become properties.
        if kind == SymbolKind::Record
            && let Some(params) = first_child_of_kind(node, "parameter_list") {
                for p in children_of_kind(params, "parameter") {
                    if let Some(pname) = p.child_by_field_name("name") {
                        let ty = p.child_by_field_name("type");
                        self.member_symbol(&ctx, p, SymbolKind::Property, &src.norm(pname), None, ty, vec!["positional".into()]);
                    }
                }
            }

        if kind == SymbolKind::Delegate {
            return;
        }
        let body = node.child_by_field_name("body").or_else(|| first_child_of_kind(node, "declaration_list"));
        let Some(body) = body else { return };
        for member in named_children(body) {
            self.member(member, &ctx, ns);
        }
    }

    fn member(&mut self, node: Node<'t>, ty: &TypeCtx, ns: Option<&str>) {
        let src = self.src;
        match node.kind() {
            "method_declaration" | "constructor_declaration" | "destructor_declaration"
            | "operator_declaration" | "conversion_operator_declaration" => self.method(node, ty),
            "property_declaration" | "indexer_declaration" => self.property(node, ty),
            "field_declaration" | "event_field_declaration" => {
                let kind = if node.kind() == "event_field_declaration" { SymbolKind::Event } else { SymbolKind::Field };
                let Some(decl) = first_child_of_kind(node, "variable_declaration") else { return };
                let ty_node = decl.child_by_field_name("type");
                let mut flavors = modifiers(src, node);
                flavors.extend(attributes(src, node));
                if kind == SymbolKind::Field && ty.unity && unity::is_inspector_field(&flavors) {
                    flavors.push("unity:inspector".into());
                }
                for declarator in children_of_kind(decl, "variable_declarator") {
                    if let Some(name) = declarator.child_by_field_name("name") {
                        self.member_symbol(ty, declarator, kind, &src.norm(name), Some(node), ty_node, flavors.clone());
                    }
                }
            }
            "event_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let mut flavors = modifiers(src, node);
                    flavors.extend(attributes(src, node));
                    self.member_symbol(ty, node, SymbolKind::Event, &src.norm(name), Some(node), node.child_by_field_name("type"), flavors);
                }
            }
            "enum_member_declaration_list" => {
                for m in children_of_kind(node, "enum_member_declaration") {
                    if let Some(name) = m.child_by_field_name("name") {
                        self.member_symbol(ty, m, SymbolKind::EnumMember, &src.norm(name), None, None, Vec::new());
                    }
                }
            }
            "enum_member_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.member_symbol(ty, node, SymbolKind::EnumMember, &src.norm(name), None, None, Vec::new());
                }
            }
            _ => self.declaration(node, ns, Some(ty)),
        }
    }

    fn params(&self, node: Node) -> Vec<(String, Option<String>, Vec<String>)> {
        let src = self.src;
        let Some(list) = node.child_by_field_name("parameters").or_else(|| first_child_of_kind(node, "parameter_list")) else {
            return Vec::new();
        };
        children_of_kind(list, "parameter")
            .into_iter()
            .map(|p| {
                let name = p.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
                let ty = p.child_by_field_name("type").map(|t| src.norm(t));
                let mods = children(p)
                    .into_iter()
                    .filter(|c| c.kind() == "modifier" || c.kind() == "parameter_modifier" || (!c.is_named() && matches!(src.text(*c), "this" | "ref" | "out" | "in" | "params")))
                    .map(|c| src.norm(c))
                    .collect();
                (name, ty, mods)
            })
            .collect()
    }

    fn method(&mut self, node: Node<'t>, ty: &TypeCtx) {
        let src = self.src;
        let params = self.params(node);
        let param_types: Vec<String> = params.iter().map(|(_, t, _)| t.clone().unwrap_or_else(|| "?".into())).collect();
        let (kind, name, id_name) = match node.kind() {
            "constructor_declaration" => (SymbolKind::Constructor, ty.name.clone(), ".ctor".to_owned()),
            "destructor_declaration" => (SymbolKind::Method, format!("~{}", ty.name), "Finalize".to_owned()),
            "operator_declaration" => {
                let op = node.child_by_field_name("operator").map(|o| src.norm(o)).unwrap_or_else(|| "?".into());
                (SymbolKind::Method, format!("operator {op}"), format!("op_{op}"))
            }
            "conversion_operator_declaration" => {
                let target = node.child_by_field_name("type").map(|t| src.norm(t)).unwrap_or_default();
                (SymbolKind::Method, format!("operator {target}"), format!("op_Convert_{target}"))
            }
            _ => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
                (SymbolKind::Method, name.clone(), name)
            }
        };
        let id = format!("{}.{}({})", ty.qualified, id_name, param_types.join(", "));
        let returns = node.child_by_field_name("returns").or_else(|| node.child_by_field_name("type"));
        let body = node.child_by_field_name("body").or_else(|| {
            named_children(node).into_iter().find(|c| matches!(c.kind(), "block" | "arrow_expression_clause"))
        });

        let mut flavors = modifiers(src, node);
        flavors.extend(attributes(src, node));
        if params.first().is_some_and(|(_, _, m)| m.iter().any(|m| m == "this")) {
            flavors.push("extension".into());
        }
        let unity_message = if ty.unity && kind == SymbolKind::Method {
            unity::lifecycle_message(&name)
        } else {
            None
        };
        if unity_message.is_some() {
            flavors.push("unity:message".into());
        }
        if let Some(b) = body
            && contains_kind(b, "yield_statement") {
                flavors.push("iterator".into());
                if ty.unity || self.unity_file {
                    let ret = returns.map(|r| src.norm(r)).unwrap_or_default();
                    if ret == "IEnumerator" || ret.ends_with(".IEnumerator") {
                        flavors.push("unity:coroutine".into());
                    }
                }
            }
        if ty.kind.is_contract() && body.is_none() {
            flavors.push("abstract".into());
        }

        let signature = src.header(node, body);
        let type_text = returns.map(|r| src.norm(r));
        self.push_or_merge(Symbol {
            id: id.clone(),
            kind,
            name: name.clone(),
            qualified_name: format!("{}.{}", ty.qualified, name),
            parent: Some(ty.id.clone()),
            language: Language::Csharp,
            origins: vec![src.span(node)],
            flavors: flavors.clone(),
            signature: Some(signature.trim_end_matches(';').trim().to_owned()),
            type_text: type_text.clone(),
            has_flow: false,
            external: false,
        });
        for (_, t, _) in &params {
            if let Some(t) = t {
                self.refs_from_text(&id, t);
            }
        }
        if let Some(r) = returns {
            for n in type_names(src, r) {
                self.type_refs.push(TypeRef { from: id.clone(), name: n, relation: TypeRelation::References });
            }
        }

        if let Some(body) = body {
            let returns_void = matches!(kind, SymbolKind::Constructor)
                || node.kind() == "destructor_declaration"
                || type_text.as_deref() == Some("void");
            self.bodies.push(BodyJob {
                symbol: id,
                name,
                kind: if kind == SymbolKind::Constructor { BodyKind::Constructor } else { BodyKind::Method },
                decl: node,
                body,
                signature,
                params: params.into_iter().map(|(n, t, _)| (n, t)).collect(),
                returns_void,
                flavors,
                unity_message,
                unity: ty.unity || self.unity_file,
            });
        }
    }

    fn property(&mut self, node: Node<'t>, ty: &TypeCtx) {
        let src = self.src;
        let is_indexer = node.kind() == "indexer_declaration";
        let name = if is_indexer {
            "this[]".to_owned()
        } else {
            node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into())
        };
        let ty_node = node.child_by_field_name("type");
        let mut flavors = modifiers(src, node);
        flavors.extend(attributes(src, node));
        if is_indexer {
            flavors.push("indexer".into());
        }
        let id = self.member_symbol(ty, node, SymbolKind::Property, &name, Some(node), ty_node, flavors.clone());

        let type_text = ty_node.map(|t| src.norm(t));
        // Expression-bodied property: `public bool IsLarge => Amount >= 100m;`
        let arrow = node
            .child_by_field_name("value")
            .filter(|v| v.kind() == "arrow_expression_clause")
            .or_else(|| first_child_of_kind(node, "arrow_expression_clause"));
        if let Some(arrow) = arrow {
            flavors.push("computed".into());
            if let Some(sym) = self.symbols.iter_mut().find(|s| s.id == id) {
                sym.flavors.push("computed".into());
            }
            self.bodies.push(BodyJob {
                symbol: id.clone(),
                name: name.clone(),
                kind: BodyKind::Getter,
                decl: node,
                body: arrow,
                signature: src.header(node, Some(arrow)),
                params: Vec::new(),
                returns_void: false,
                flavors,
                unity_message: None,
                unity: ty.unity,
            });
            return;
        }

        let Some(accessors) = node.child_by_field_name("accessors").or_else(|| first_child_of_kind(node, "accessor_list")) else {
            return;
        };
        for acc in children_of_kind(accessors, "accessor_declaration") {
            let keyword = children(acc)
                .into_iter()
                .map(|c| src.text(c))
                .find(|t| matches!(*t, "get" | "set" | "init" | "add" | "remove"))
                .unwrap_or("get");
            let Some(body) = acc.child_by_field_name("body").or_else(|| {
                named_children(acc).into_iter().find(|c| matches!(c.kind(), "block" | "arrow_expression_clause"))
            }) else {
                continue; // auto-accessor
            };
            let acc_id = format!("{id}.{keyword}");
            let role = if keyword == "get" { "getter" } else { "setter" };
            let mut acc_flavors = modifiers(src, acc);
            acc_flavors.push(role.into());
            self.symbols.push(Symbol {
                id: acc_id.clone(),
                kind: SymbolKind::Method,
                name: format!("{name}.{keyword}"),
                qualified_name: format!("{}.{}.{}", ty.qualified, name, keyword),
                parent: Some(id.clone()),
                language: Language::Csharp,
                origins: vec![src.span(acc)],
                flavors: acc_flavors.clone(),
                signature: Some(format!("{keyword} accessor of {name}")),
                type_text: type_text.clone(),
                has_flow: false,
                external: false,
            });
            let params = if keyword == "get" {
                Vec::new()
            } else {
                vec![("value".to_owned(), type_text.clone())]
            };
            self.bodies.push(BodyJob {
                symbol: acc_id,
                name: format!("{name}.{keyword}"),
                kind: if keyword == "get" { BodyKind::Getter } else { BodyKind::Setter },
                decl: acc,
                body,
                signature: format!("{keyword} {name}"),
                params,
                returns_void: keyword != "get",
                flavors: acc_flavors,
                unity_message: None,
                unity: ty.unity,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn member_symbol(
        &mut self,
        ty: &TypeCtx,
        node: Node,
        kind: SymbolKind,
        name: &str,
        sig_node: Option<Node>,
        ty_node: Option<Node>,
        flavors: Vec<String>,
    ) -> SymbolId {
        let src = self.src;
        let id = format!("{}.{}", ty.qualified, name);
        let type_text = ty_node.map(|t| src.norm(t));
        let signature = sig_node.map(|s| {
            let body = s
                .child_by_field_name("accessors")
                .or_else(|| s.child_by_field_name("value"))
                .or_else(|| first_child_of_kind(s, "accessor_list"))
                .or_else(|| first_child_of_kind(s, "arrow_expression_clause"));
            src.header(s, body).trim_end_matches(';').trim().to_owned()
        });
        self.push_or_merge(Symbol {
            id: id.clone(),
            kind,
            name: name.to_owned(),
            qualified_name: id.clone(),
            parent: Some(ty.id.clone()),
            language: Language::Csharp,
            origins: vec![src.span(node)],
            flavors,
            signature,
            type_text,
            has_flow: false,
            external: false,
        });
        if let Some(t) = ty_node {
            for n in type_names(src, t) {
                self.type_refs.push(TypeRef { from: id.clone(), name: n, relation: TypeRelation::References });
            }
        }
        id
    }

    fn refs_from_text(&mut self, from: &str, type_text: &str) {
        // Parameter types are stored as text; split generic arguments heuristically.
        for part in type_text.split(['<', '>', ',', '?', '[', ']', '(', ')', ' ']) {
            let part = part.trim();
            if part.is_empty() || part.chars().next().is_some_and(|c| c.is_lowercase()) {
                continue; // predefined types (`int`, `string`) and modifiers
            }
            self.type_refs.push(TypeRef { from: from.to_owned(), name: part.to_owned(), relation: TypeRelation::References });
        }
    }

    fn top_level_program(&mut self, root: Node<'t>, ns: Option<&str>) {
        let src = self.src;
        let qualified = match ns {
            Some(ns) => format!("{ns}.Program"),
            None => "Program".to_owned(),
        };
        let parent = ns.map(|ns| format!("ns:{ns}"));
        if !self.symbols.iter().any(|s| s.id == qualified) {
            self.symbols.push(Symbol {
                id: qualified.clone(),
                kind: SymbolKind::Class,
                name: "Program".into(),
                qualified_name: qualified.clone(),
                parent,
                language: Language::Csharp,
                origins: vec![src.span(root)],
                flavors: vec!["static".into(), "synthesized".into()],
                signature: None,
                type_text: None,
                has_flow: false,
                external: false,
            });
        }
        let id = format!("{qualified}.<Main>$(string[])");
        self.symbols.push(Symbol {
            id: id.clone(),
            kind: SymbolKind::Method,
            name: "Top-level statements".into(),
            qualified_name: format!("{qualified}.<Main>$"),
            parent: Some(qualified),
            language: Language::Csharp,
            origins: vec![src.span(root)],
            flavors: vec!["static".into(), "top-level".into()],
            signature: Some("top-level statements".into()),
            type_text: None,
            has_flow: false,
            external: false,
        });
        self.bodies.push(BodyJob {
            symbol: id,
            name: "Top-level statements".into(),
            kind: BodyKind::TopLevel,
            decl: root,
            body: root,
            signature: "top-level statements".into(),
            params: vec![("args".into(), Some("string[]".into()))],
            returns_void: true,
            flavors: vec!["top-level".into()],
            unity_message: None,
            unity: false,
        });
    }

    /// Adds a symbol, merging partial declarations that share an id.
    fn push_or_merge(&mut self, symbol: Symbol) {
        if let Some(existing) = self.symbols.iter_mut().find(|s| s.id == symbol.id) {
            existing.origins.extend(symbol.origins);
            for f in symbol.flavors {
                if !existing.flavors.contains(&f) {
                    existing.flavors.push(f);
                }
            }
        } else {
            self.symbols.push(symbol);
        }
    }
}

/// The type currently being visited.
pub struct TypeCtx {
    pub id: SymbolId,
    pub qualified: String,
    pub name: String,
    pub kind: SymbolKind,
    /// True when the type is (probably) a Unity behaviour.
    pub unity: bool,
}
