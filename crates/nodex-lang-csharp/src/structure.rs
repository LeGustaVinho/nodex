//! Declarations → structure symbols, imports and type references.

use nodex_ir::{ImportRef, Language, Symbol, SymbolId, SymbolKind, TypeRef, TypeRelation};
use tree_sitter::Node;

use std::collections::HashMap;

use crate::syntax::{
    Src, attributes, bare_type_name, children, children_of_kind, contains_kind, doc_comment, first_child_of_kind,
    flatten_preproc, id_types, local_function_id, modifiers, named_children, parameters, type_names,
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
    /// Event `add` accessor: runs when a handler subscribes.
    EventAdd,
    /// Event `remove` accessor: runs when a handler unsubscribes.
    EventRemove,
    /// A function declared inside another function.
    LocalFunction,
}

/// A function-like body waiting to be lowered into a flow graph.
pub struct BodyJob<'t> {
    pub symbol: SymbolId,
    /// The member that owns the body (local functions belong to their enclosing method).
    pub owner: SymbolId,
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
    /// Build conditions (`#if DEBUG`) around the declaration being visited.
    conditions: Vec<String>,
    /// C# 14 `extension(T receiver)` blocks, rewritten for the grammar: start byte → receiver.
    extension_blocks: HashMap<usize, String>,
}

const GLOBAL_NS: &str = "(global)";

impl<'a, 't> Collector<'a, 't> {
    pub fn new(src: Src<'a>, extension_blocks: HashMap<usize, String>) -> Self {
        Self {
            src,
            symbols: Vec::new(),
            type_refs: Vec::new(),
            bodies: Vec::new(),
            usings: Vec::new(),
            namespaces: Vec::new(),
            unity_file: false,
            conditions: Vec::new(),
            extension_blocks,
        }
    }

    /// Visits `nodes` with `#if` branches flattened, tracking their build conditions.
    fn each_conditional(&mut self, nodes: Vec<Node<'t>>, mut visit: impl FnMut(&mut Self, Node<'t>)) {
        let mut flat = Vec::new();
        flatten_preproc(self.src, nodes, &self.conditions.clone(), &mut flat);
        let saved = self.conditions.clone();
        for (node, conds) in flat {
            self.conditions = conds;
            visit(self, node);
        }
        self.conditions = saved;
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
        self.each_conditional(named_children(root), |this, child| match child.kind() {
            "file_scoped_namespace_declaration" => {
                let name = child.child_by_field_name("name").map(|n| this.src.norm(n)).unwrap_or_default();
                this.namespace_symbol(&name, child);
                // Older grammars nest the members; newer ones make them siblings.
                this.declarations(child, Some(&name), None);
                current_ns = Some(name);
            }
            "namespace_declaration" => this.namespace(child, None),
            "global_statement" => top_level.push(child),
            _ => this.declaration(child, current_ns.as_deref(), None),
        });
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
            doc: None,
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
            self.each_conditional(named_children(body), |this, child| {
                if child.kind() == "namespace_declaration" {
                    this.namespace(child, Some(&name));
                } else {
                    this.declaration(child, Some(&name), None);
                }
            });
        }
    }

    /// Visits the declarations nested directly in `node` (used for file-scoped namespaces).
    fn declarations(&mut self, node: Node<'t>, ns: Option<&str>, parent_type: Option<&TypeCtx>) {
        let kids: Vec<Node<'t>> = named_children(node).into_iter().filter(|c| c.kind() != "qualified_name" && c.kind() != "identifier").collect();
        self.each_conditional(kids, |this, child| this.declaration(child, ns, parent_type));
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
        // C# 14 extension block (`extension(string s) { ... }`): its members extend the receiver
        // and belong to the enclosing static class.
        if let (Some(receiver), Some(owner)) = (self.extension_blocks.get(&node.start_byte()).cloned(), parent_type) {
            let saved = self.conditions.clone();
            self.conditions.push("extension".into());
            self.conditions.push(format!("extends:{receiver}"));
            if let Some(body) = node.child_by_field_name("body").or_else(|| first_child_of_kind(node, "declaration_list")) {
                let members = named_children(body);
                self.each_conditional(members, |this, member| this.member(member, owner, ns));
            }
            self.conditions = saved;
            return;
        }
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
        if text_has("ref") {
            flavors.push("ref-struct".into());
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
            doc: doc_comment(src, node),
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

        // Primary constructor (`class Square(double side)`, records): a constructor without a body.
        if matches!(kind, SymbolKind::Class | SymbolKind::Struct | SymbolKind::Record)
            && let Some(list) = first_child_of_kind(node, "parameter_list")
        {
            let params = parameters(src, node);
            let ctor_id = format!("{}..ctor({})", ctx.qualified, id_types(&params).join(", "));
            self.push_or_merge(Symbol {
                id: ctor_id,
                kind: SymbolKind::Constructor,
                name: ctx.name.clone(),
                qualified_name: format!("{}.{}", ctx.qualified, ctx.name),
                parent: Some(ctx.id.clone()),
                language: Language::Csharp,
                origins: vec![src.span(list)],
                flavors: vec!["primary-constructor".into()],
                signature: Some(format!("{}{}", ctx.name, src.norm(list))),
                type_text: None,
                has_flow: false,
                external: false,
                doc: None,
            });
        }

        if kind == SymbolKind::Delegate {
            return;
        }
        let body = node.child_by_field_name("body").or_else(|| first_child_of_kind(node, "declaration_list"));
        let Some(body) = body else { return };
        self.each_conditional(named_children(body), |this, member| this.member(member, &ctx, ns));
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
                    let ty_node = node.child_by_field_name("type");
                    let name = src.norm(name);
                    let id = self.member_symbol(ty, node, SymbolKind::Event, &name, Some(node), ty_node, flavors);
                    self.accessors(node, ty, &id, &name, ty_node.map(|t| src.norm(t)));
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
        parameters(self.src, node)
    }

    /// `IEnumerable.` for an explicit interface implementation (`IEnumerable.GetEnumerator()`).
    fn explicit_interface(&self, node: Node) -> String {
        first_child_of_kind(node, "explicit_interface_specifier")
            .map(|s| {
                let full = bare_type_name(self.src.text(s).trim_end_matches('.'));
                format!("{}.", full.rsplit('.').next().unwrap_or(&full))
            })
            .unwrap_or_default()
    }

    /// Function declarations nested in a body get their own symbols and Blueprints.
    fn local_functions(&mut self, body: Node<'t>, owner: &str, parent: &str, ty: &TypeCtx) {
        let src = self.src;
        let mut stack = vec![body];
        while let Some(n) = stack.pop() {
            for child in named_children(n) {
                if matches!(child.kind(), "lambda_expression" | "anonymous_method_expression") {
                    self.callback(child, owner, parent, ty);
                    stack.push(child);
                    continue;
                }
                if child.kind() != "local_function_statement" {
                    stack.push(child);
                    continue;
                }
                stack.push(child);
                let name = child.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
                let Some(fn_body) = child.child_by_field_name("body").or_else(|| {
                    named_children(child).into_iter().find(|c| matches!(c.kind(), "block" | "arrow_expression_clause"))
                }) else {
                    continue;
                };
                let id = local_function_id(src, owner, child);
                let params = parameters(src, child);
                let mut flavors = modifiers(src, child);
                flavors.push("local-function".into());
                if contains_kind(fn_body, "yield_statement") {
                    flavors.push("iterator".into());
                }
                let returns = child.child_by_field_name("type").map(|t| src.norm(t));
                let signature = src.header(child, Some(fn_body));
                self.push_or_merge(Symbol {
                    id: id.clone(),
                    kind: SymbolKind::Function,
                    name: name.clone(),
                    qualified_name: format!("{parent}.{name}"),
                    parent: Some(parent.to_owned()),
                    language: Language::Csharp,
                    origins: vec![src.span(child)],
                    flavors: flavors.clone(),
                    signature: Some(signature.clone()),
                    type_text: returns.clone(),
                    has_flow: false,
                    external: false,
                    doc: None,
                });
                self.bodies.push(BodyJob {
                    symbol: id,
                    owner: owner.to_owned(),
                    name,
                    kind: BodyKind::LocalFunction,
                    decl: child,
                    body: fn_body,
                    signature,
                    params: params.into_iter().map(|(n, t, _)| (n, t)).collect(),
                    returns_void: returns.as_deref() == Some("void"),
                    flavors,
                    unity_message: None,
                    unity: ty.unity || self.unity_file,
                });
            }
        }
    }

    /// A block-bodied lambda handed to a call (`items.ForEach(item => { … })`) or to an event
    /// (`button.Click += (s, e) => { … }`) gets a Blueprint of its own: `<callee> callback`.
    fn callback(&mut self, lambda: Node<'t>, owner: &str, parent: &str, ty: &TypeCtx) {
        let src = self.src;
        let Some(body) = lambda.child_by_field_name("body").or_else(|| first_child_of_kind(lambda, "block")) else { return };
        if body.kind() != "block" || !named_children(body).iter().any(|s| s.kind() != "comment") {
            return;
        }
        let Some(holder) = lambda.parent() else { return };
        let name = match holder.kind() {
            "argument" => {
                let invocation = holder.parent().and_then(|list| list.parent());
                let callee = invocation
                    .filter(|i| matches!(i.kind(), "invocation_expression" | "object_creation_expression"))
                    .map(|i| match i.kind() {
                        "object_creation_expression" => {
                            i.child_by_field_name("type").map(|t| bare_type_name(&src.norm(t))).unwrap_or_default()
                        }
                        _ => match i.child_by_field_name("function") {
                            Some(f) if f.kind() == "member_access_expression" => {
                                f.child_by_field_name("name").map(|n| bare_type_name(&src.norm(n))).unwrap_or_default()
                            }
                            Some(f) => bare_type_name(&src.norm(f)),
                            None => String::new(),
                        },
                    })
                    .filter(|n| !n.is_empty() && !n.contains(['(', '.']))
                    .unwrap_or_else(|| "call".into());
                format!("{callee} callback")
            }
            "assignment_expression"
                if holder.child_by_field_name("right").is_some_and(|r| r.id() == lambda.id())
                    && holder.child_by_field_name("operator").is_some_and(|o| src.text(o) == "+=") =>
            {
                let event = holder
                    .child_by_field_name("left")
                    .map(|l| src.norm(l))
                    .map(|l| l.rsplit('.').next().unwrap_or(&l).to_owned())
                    .unwrap_or_else(|| "event".into());
                format!("{event} handler")
            }
            _ => return,
        };
        let mut id = format!("{owner}.{name}");
        let mut i = 2;
        while self.symbols.iter().any(|s| s.id == id) {
            id = format!("{owner}.{name}~{i}");
            i += 1;
        }
        let mut params: Vec<(String, Option<String>)> = parameters(src, lambda).into_iter().map(|(n, t, _)| (n, t)).collect();
        if params.is_empty()
            && let Some(p) = lambda.child_by_field_name("parameters").filter(|p| matches!(p.kind(), "identifier" | "implicit_parameter"))
        {
            params.push((src.norm(p), None));
        }
        let mut flavors = vec!["local-function".to_owned(), "callback".to_owned()];
        if children(lambda).iter().any(|c| src.text(*c) == "async") {
            flavors.push("async".into());
        }
        let signature = src.header(lambda, Some(body));
        self.push_or_merge(Symbol {
            id: id.clone(),
            kind: SymbolKind::Function,
            name: name.clone(),
            qualified_name: format!("{parent}.{name}"),
            parent: Some(parent.to_owned()),
            language: Language::Csharp,
            origins: vec![src.span(lambda)],
            flavors: flavors.clone(),
            signature: Some(signature.clone()),
            type_text: None,
            has_flow: false,
            external: false,
            doc: None,
        });
        self.bodies.push(BodyJob {
            symbol: id,
            owner: owner.to_owned(),
            name,
            kind: BodyKind::LocalFunction,
            decl: lambda,
            body,
            signature,
            params,
            returns_void: false,
            flavors,
            unity_message: None,
            unity: ty.unity || self.unity_file,
        });
    }

    fn method(&mut self, node: Node<'t>, ty: &TypeCtx) {
        let src = self.src;
        let params = self.params(node);
        let param_types = id_types(&params);
        let is_static = modifiers(src, node).iter().any(|m| m == "static");
        let explicit = self.explicit_interface(node);
        let (kind, name, id_name) = match node.kind() {
            "constructor_declaration" if is_static => (SymbolKind::Constructor, format!("static {}", ty.name), ".cctor".to_owned()),
            "constructor_declaration" => (SymbolKind::Constructor, ty.name.clone(), ".ctor".to_owned()),
            "destructor_declaration" => (SymbolKind::Method, format!("~{}", ty.name), "Finalize".to_owned()),
            "operator_declaration" => {
                let op_node = node.child_by_field_name("operator");
                let mut op = op_node.map(|o| src.norm(o)).unwrap_or_else(|| "?".into());
                // C# 14 `operator +=` is parsed as `+` (see prepare.rs); the original has the `=`.
                if let Some(o) = op_node
                    && src.text.get(o.end_byte()..o.end_byte() + 1) == Some("=")
                {
                    op.push('=');
                }
                // `operator checked +` is a distinct overload of `operator +`.
                let checked = if children(node).iter().any(|c| src.text(*c) == "checked") { "checked " } else { "" };
                (SymbolKind::Method, format!("operator {checked}{op}"), format!("op_{}{op}", checked.replace(' ', "_")))
            }
            "conversion_operator_declaration" => {
                let target = node.child_by_field_name("type").map(|t| src.norm(t)).unwrap_or_default();
                (SymbolKind::Method, format!("operator {target}"), format!("op_Convert_{target}"))
            }
            _ => {
                let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
                let name = format!("{explicit}{name}");
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
        flavors.extend(self.conditions.iter().cloned());
        if params.first().is_some_and(|(_, _, m)| m.iter().any(|m| m == "this")) {
            flavors.push("extension".into());
        }
        if !explicit.is_empty() {
            flavors.push("explicit-interface".into());
        }
        if flavors.iter().any(|f| f == "async")
            && returns.is_some_and(|r| src.norm(r) == "void")
        {
            flavors.push("async-void".into());
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
            doc: doc_comment(src, node),
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
            self.local_functions(body, &id, &id, ty);
            self.bodies.push(BodyJob {
                owner: id.clone(),
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
        let explicit = self.explicit_interface(node);
        let name = if is_indexer {
            format!("{explicit}this[]")
        } else {
            let short = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
            format!("{explicit}{short}")
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
                owner: id.clone(),
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

        self.accessors(node, ty, &id, &name, type_text);
    }

    /// `get`/`set`/`init` of properties and indexers, `add`/`remove` of events: one Blueprint
    /// per accessor that has a body.
    fn accessors(&mut self, node: Node<'t>, ty: &TypeCtx, id: &str, name: &str, type_text: Option<String>) {
        let src = self.src;
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
            let role = match keyword {
                "get" => "getter",
                "add" => "event-add",
                "remove" => "event-remove",
                _ => "setter",
            };
            let mut acc_flavors = modifiers(src, acc);
            acc_flavors.push(role.into());
            self.symbols.push(Symbol {
                id: acc_id.clone(),
                kind: SymbolKind::Method,
                name: format!("{name}.{keyword}"),
                qualified_name: format!("{}.{}.{}", ty.qualified, name, keyword),
                parent: Some(id.to_owned()),
                language: Language::Csharp,
                origins: vec![src.span(acc)],
                flavors: acc_flavors.clone(),
                signature: Some(format!("{keyword} accessor of {name}")),
                type_text: type_text.clone(),
                has_flow: false,
                external: false,
                doc: None,
            });
            let params = if keyword == "get" {
                Vec::new()
            } else {
                vec![("value".to_owned(), type_text.clone())]
            };
            self.bodies.push(BodyJob {
                owner: acc_id.clone(),
                symbol: acc_id,
                name: format!("{name}.{keyword}"),
                kind: match keyword {
                    "get" => BodyKind::Getter,
                    "add" => BodyKind::EventAdd,
                    "remove" => BodyKind::EventRemove,
                    _ => BodyKind::Setter,
                },
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
            doc: doc_comment(src, sig_node.unwrap_or(node)),
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
                doc: None,
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
            doc: None,
        });
        self.local_functions(root, &id, &id, &TypeCtx { id: qualified_id(&id), qualified: String::new(), name: String::new(), kind: SymbolKind::Class, unity: false });
        self.bodies.push(BodyJob {
            owner: id.clone(),
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

    /// Adds a symbol, merging partial declarations that share an id. Build conditions around
    /// the declaration (`#if DEBUG`) become flavors.
    fn push_or_merge(&mut self, mut symbol: Symbol) {
        for c in &self.conditions {
            if !symbol.flavors.contains(c) {
                symbol.flavors.push(c.clone());
            }
        }
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

/// The type part of a member id (`A.B.M(int)` → `A.B`).
fn qualified_id(member: &str) -> String {
    let head = member.split('(').next().unwrap_or(member);
    head.rsplit_once('.').map(|(t, _)| t.to_owned()).unwrap_or_default()
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
