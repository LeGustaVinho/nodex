use crate::syntax::{Src, children};
use nodex_ir::{Concept, Expr, FileIr, Language, Span, SymbolKind, SyntaxSite};
use tree_sitter::Node;

struct Occurrence {
    site: SyntaxSite,
    owner: Span,
    operand: Option<Span>,
    declaration: bool,
    field_modifier: bool,
    statement: bool,
}

/// The Rust grammar parses `..=end` as `..` assigned to `end`. Recognize the joined operator.
pub fn rust_inclusive_range_to(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if node.kind() != "assignment_expression" {
        return None;
    }
    let dots = node.child_by_field_name("left")?;
    if dots.kind() != "range_expression" || dots.end_byte() - dots.start_byte() != 2 {
        return None;
    }
    let equal = children(node)
        .into_iter()
        .find(|token| token.kind() == "=" && token.start_byte() == dots.end_byte())?;
    let end = node.child_by_field_name("right")?;
    Some((equal, end))
}

pub fn annotate(ir: &mut FileIr, root: Node, src: Src, javascript: bool) {
    let mut occurrences = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        // Token trees are macro input, and extras and missing nodes aren't written syntax.
        if node.is_extra() || node.is_error() || node.is_missing() || node.kind() == "token_tree" {
            continue;
        }
        let mut add = |key: &str,
                       token: Node,
                       owner: Node,
                       declaration: bool,
                       statement: bool,
                       pin: Option<String>| {
            if token.start_byte() == token.end_byte() || src.text(token).trim().is_empty() {
                return;
            }
            let operand = match owner.kind() {
                "non_null_expression"
                | "postfix_unary_expression"
                | "try_expression"
                | "unary_expression" => crate::syntax::named_children(owner).into_iter().next(),
                "reference_expression" => owner.child_by_field_name("value"),
                "match_expression" if pin.as_deref().is_some_and(|pin| pin.starts_with("case:")) => {
                    std::iter::successors(token.parent(), Node::parent)
                        .find(|ancestor| ancestor.kind() == "match_arm")
                }
                _ => None,
            }
            .map(|n| src.span(n));
            occurrences.push(Occurrence {
                site: SyntaxSite {
                    key: key.into(),
                    span: syntax_span(ir.language, key, src, token, owner),
                    pin,
                },
                owner: src.span(owner),
                operand,
                declaration,
                field_modifier: declaration && owner.kind() == "field_declaration",
                statement,
            });
        };
        collect(ir.language, javascript, src, node, &mut add);
        stack.extend(children(node).into_iter().rev());
    }
    occurrences.sort_by_key(|o| (o.site.span.start_byte, o.site.span.end_byte));
    occurrences.dedup_by(|a, b| a.site == b.site && a.owner == b.owner);
    let field_modifiers: Vec<_> = occurrences.iter().filter(|o| o.field_modifier).collect();
    let mut leading = std::collections::HashMap::<u32, Vec<&Occurrence>>::new();
    for occurrence in &occurrences {
        if occurrence.site.span.start_byte < occurrence.owner.start_byte {
            leading.entry(occurrence.owner.start_byte).or_default().push(occurrence);
        }
    }

    // Each declaration gets only its header's details; a method body belongs to its flow.
    for symbol in &mut ir.symbols {
        let mut sites = Vec::new();
        for origin in &symbol.origins {
            if let Some(attributes) = leading.get(&origin.start_byte) {
                sites.extend(attributes.iter().map(|occurrence| occurrence.site.clone()));
            }
            // C# field symbols can start at the declarator, after the shared modifiers.
            if symbol.kind == SymbolKind::Field {
                let end =
                    field_modifiers.partition_point(|o| o.owner.start_byte <= origin.start_byte);
                for o in field_modifiers[..end]
                    .iter()
                    .rev()
                    .take_while(|o| o.owner.end_byte >= origin.end_byte)
                {
                    sites.push(o.site.clone());
                }
            }
            for o in in_span(&occurrences, origin).filter(|o| o.declaration) {
                if (o.site.key == "type-import") != (symbol.kind == SymbolKind::Module) {
                    continue;
                }
                if symbol.kind == SymbolKind::Module
                    || o.owner.start_byte == origin.start_byte
                    || (o.owner.end_byte >= origin.end_byte
                        && origin.start_byte <= o.site.span.start_byte)
                {
                    if !sites.contains(&o.site) {
                        sites.push(o.site.clone());
                    }
                }
            }
        }
        if !sites.is_empty() {
            symbol.syntax_sites = Some(sites);
        }
    }

    for flow in &mut ir.flows {
        for node in &mut flow.nodes {
            if let Some(attributes) = leading.get(&node.span.start_byte) {
                node.syntax_sites.get_or_insert_with(Vec::new)
                    .extend(attributes.iter().map(|occurrence| occurrence.site.clone()));
            }
        }
        let sites: Vec<_> = in_span(&occurrences, &flow.span)
            .filter(|o| o.site.key != "type-import")
            .collect();
        let mut targets: Vec<Option<usize>> = vec![None; sites.len()];
        for (i, node) in flow
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !node.flavors.iter().any(|f| f == "implicit-drop"))
        {
            let start = sites.partition_point(|o| o.site.span.start_byte < node.span.start_byte);
            for (j, o) in sites
                .iter()
                .enumerate()
                .skip(start)
                .take_while(|(_, o)| o.site.span.start_byte < node.span.end_byte)
            {
                if o.site.span.end_byte > node.span.end_byte {
                    continue;
                }
                let size = |s: &Span| s.end_byte - s.start_byte;
                if targets[j]
                    .is_none_or(|previous| size(&node.span) < size(&flow.nodes[previous].span))
                {
                    targets[j] = Some(i);
                }
            }
        }
        for (o, target) in sites.iter().zip(&mut targets) {
            if match_case_site(o)
                && let Some(match_node) = flow.nodes.iter().position(|n| {
                    n.concept == Concept::Match && n.span.start_byte == o.owner.start_byte
                })
            {
                *target = Some(match_node);
            }
        }
        for (o, i) in sites
            .into_iter()
            .zip(targets)
            .filter_map(|(o, i)| i.map(|i| (o, i)))
        {
            let node = &mut flow.nodes[i];
            if o.declaration && !matches!(node.concept, Concept::Entry | Concept::EventEntry) {
                continue;
            }
            let case_in_expression = match_case_site(o)
                && (node.concept != Concept::Match || node.span.start_byte != o.owner.start_byte);
            let mut site = o.site.clone();
            site.pin = site.pin.filter(|pin| {
                !case_in_expression
                    && node.inputs.iter().chain(&node.outputs).any(|p| &p.id == pin)
            });
            if (case_in_expression || !o.statement && o.site.pin.is_none())
                && node.expr.as_mut().is_some_and(|e| attach_expr(e, o, &site))
            {
                continue;
            }
            node.syntax_sites.get_or_insert_with(Vec::new).push(site);
        }
        for node in &mut flow.nodes {
            if node.flavors.iter().any(|f| f == "implicit-return")
                && node.span.start_byte < node.span.end_byte
            {
                let span = node.expr.as_ref().map_or(&node.span, |e| &e.span).clone();
                node.syntax_sites
                    .get_or_insert_with(Vec::new)
                    .push(SyntaxSite {
                        key: "tail-expression".into(),
                        span,
                        pin: None,
                    });
            }
        }
    }
}

fn match_case_site(occurrence: &Occurrence) -> bool {
    occurrence.site.pin.as_deref().is_some_and(|pin| pin.starts_with("case:"))
}

fn rust_match_case(pattern: Node<'_>) -> Option<(Node<'_>, String)> {
    let arm = pattern.parent().filter(|parent| parent.kind() == "match_arm")?;
    let block = arm.parent()?;
    let owner = block.parent().filter(|parent| parent.kind() == "match_expression")?;
    let index = std::iter::successors(arm.prev_named_sibling(), Node::prev_named_sibling)
        .filter(|sibling| sibling.kind() == "match_arm")
        .count();
    Some((owner, format!("case:{index}")))
}

fn rust_pattern_owner(node: Node<'_>) -> Option<(Node<'_>, Option<String>)> {
    let mut child = node;
    for parent in std::iter::successors(node.parent(), Node::parent) {
        match parent.kind() {
            "match_pattern" if parent.child_by_field_name("condition") != Some(child) => {
                return rust_match_case(parent).map(|(owner, pin)| (owner, Some(pin)));
            }
            "let_declaration" | "let_condition" | "for_expression" | "parameter" | "variadic_parameter"
                if parent.child_by_field_name("pattern") == Some(child) =>
            {
                return Some((parent, None));
            }
            "closure_parameters" => return parent.parent().map(|owner| (owner, None)),
            "field_pattern" if parent.child_by_field_name("pattern") == Some(child) => {}
            "tuple_pattern" | "tuple_struct_pattern" | "slice_pattern" | "struct_pattern"
            | "mut_pattern" | "ref_pattern" | "captured_pattern" | "reference_pattern" | "or_pattern" => {}
            _ => return None,
        }
        child = parent;
    }
    None
}

fn syntax_span(language: Language, key: &str, src: Src, token: Node, owner: Node) -> Span {
    let part = |kind: &str| {
        children(owner)
            .into_iter()
            .find(|n| n.kind() == kind && !n.is_extra())
    };
    let (start, end) = match (language, key) {
        (Language::Csharp, "safe-access" | "conditional-index") => {
            let binding =
                part("member_binding_expression").or_else(|| part("element_binding_expression"));
            let end = binding.map_or(token.end_byte(), |n| n.start_byte() + 1);
            (token.start_byte(), end)
        }
        (Language::Csharp, "pass-readonly") => {
            let end = token
                .parent()
                .and_then(|p| {
                    children(p)
                        .into_iter()
                        .find(|n| !n.is_extra() && src.text(*n) == "readonly")
                })
                .map_or(token.end_byte(), |readonly| readonly.end_byte());
            (token.start_byte(), end)
        }
        (Language::Rust, "borrow-mut") => (
            part("&").map_or(token.start_byte(), |n| n.start_byte()),
            token.end_byte(),
        ),
        (Language::Rust, "explicit-type-args") => (
            token.start_byte(),
            part("type_arguments").map_or(token.end_byte(), |n| n.end_byte()),
        ),
        (Language::Rust, "range-inclusive") if owner.kind() == "assignment_expression" => {
            (owner.start_byte(), token.end_byte())
        }
        (Language::Typescript, "as-const") => (
            token.start_byte(),
            part("const").map_or(token.end_byte(), |n| n.end_byte()),
        ),
        _ => return src.span(token),
    };
    if end - start > 96 || src.text.get(start..end).is_none() {
        return src.span(token);
    }
    src.index.span(src.file, start, end)
}

fn in_span<'a>(sites: &'a [Occurrence], span: &Span) -> impl Iterator<Item = &'a Occurrence> {
    let start = sites.partition_point(|o| o.site.span.start_byte < span.start_byte);
    sites[start..]
        .iter()
        .take_while(|o| o.site.span.start_byte < span.end_byte)
        .filter(|o| o.site.span.end_byte <= span.end_byte)
}

fn attach_expr(expr: &mut Expr, occurrence: &Occurrence, site: &SyntaxSite) -> bool {
    if expr.children.iter_mut().any(|e| attach_expr(e, occurrence, site)) {
        return true;
    }
    let same = |s: &Span| s.start_byte == expr.span.start_byte && s.end_byte == expr.span.end_byte;
    if same(&occurrence.owner) || occurrence.operand.as_ref().is_some_and(same) {
        expr.syntax_sites
            .get_or_insert_with(Vec::new)
            .push(site.clone());
        return true;
    }
    false
}

fn collect(
    language: Language,
    javascript: bool,
    src: Src,
    n: Node,
    add: &mut impl FnMut(&str, Node, Node, bool, bool, Option<String>),
) {
    let kind = n.kind();
    let token = |s: &str| {
        children(n)
            .into_iter()
            .find(|c| !c.is_extra() && c.kind() == s && src.text(*c) == s)
    };
    let text = |c: Node| src.text(c);
    let mut emit = |key, t, declaration, statement| {
        if let Some(t) = t {
            add(key, t, n, declaration, statement, None);
        }
    };
    match language {
        Language::Csharp => match kind {
            "conditional_access_expression" => {
                let key = if children(n)
                    .iter()
                    .any(|c| c.kind() == "element_binding_expression")
                {
                    "conditional-index"
                } else {
                    "safe-access"
                };
                emit(key, token("?"), false, false);
            }
            "binary_expression" | "assignment_expression" => {
                for op in ["??", "??="] {
                    emit("fallback", token(op), false, false);
                }
            }
            "postfix_unary_expression" => emit("null-forgiving", token("!"), false, false),
            "prefix_unary_expression" => emit("index-from-end", token("^"), false, false),
            "range_expression" => emit("range", token(".."), false, false),
            "spread_element" => emit("collection-spread", token(".."), false, false),
            "modifier" => {
                if matches!(text(n), "readonly" | "required")
                    && let Some(parent) = n.parent()
                {
                    let key = match (text(n), parent.kind()) {
                        ("readonly", "field_declaration") => "readonly-field",
                        ("readonly", "struct_declaration" | "record_struct_declaration") => {
                            "readonly-struct"
                        }
                        (
                            "readonly",
                            "method_declaration"
                            | "property_declaration"
                            | "indexer_declaration"
                            | "accessor_declaration",
                        ) => "readonly-member",
                        (key, _) => key,
                    };
                    let owner = if key == "readonly-member"
                        && parent.kind() == "accessor_declaration"
                        && parent.child_by_field_name("body").is_none()
                        && !children(parent).iter().any(|child| {
                            matches!(child.kind(), "block" | "arrow_expression_clause")
                        }) {
                        // Auto-accessors have no separate Blueprint symbol.
                        parent
                            .parent()
                            .and_then(|list| list.parent())
                            .filter(|owner| {
                                matches!(
                                    owner.kind(),
                                    "property_declaration" | "indexer_declaration"
                                )
                            })
                            .unwrap_or(parent)
                    } else {
                        parent
                    };
                    add(key, n, owner, true, true, None);
                }
            }
            "accessor_declaration" => {
                if let Some(t) = token("init") {
                    let owner = n.parent().and_then(|p| p.parent()).unwrap_or(n);
                    add("init", t, owner, true, true, None);
                }
            }
            "argument" | "parameter" | "ref_type" | "ref_expression" => {
                let passing = children(n).into_iter().find(|c| {
                    matches!(text(*c), "ref" | "in" | "out" | "ref readonly")
                        && (c.kind() == "modifier" || !c.is_named())
                });
                if let Some(t) = passing {
                    let mode = match text(t) {
                        "in" => "pass-in",
                        "out" => "pass-out",
                        "ref readonly" => "pass-readonly",
                        "ref"
                            if children(n)
                                .iter()
                                .any(|c| !c.is_extra() && text(*c) == "readonly") =>
                        {
                            "pass-readonly"
                        }
                        _ => "pass-ref",
                    };
                    let owner = if kind == "ref_type" {
                        n.parent().unwrap_or(n)
                    } else {
                        n
                    };
                    let pin = (kind == "parameter")
                        .then(|| {
                            n.child_by_field_name("name")
                                .map(|name| format!("var:{}", text(name)))
                        })
                        .flatten();
                    add(
                        mode,
                        t,
                        owner,
                        kind == "ref_type"
                            && matches!(
                                owner.kind(),
                                "method_declaration" | "property_declaration"
                            ),
                        kind == "parameter",
                        pin,
                    );
                }
            }
            _ => {}
        },
        Language::Typescript => match kind {
            "optional_chain" => {
                if text(n) == "?." {
                    add(
                        "safe-access",
                        n,
                        n.parent().unwrap_or(n),
                        false,
                        false,
                        None,
                    );
                }
            }
            "call_expression" => emit("safe-access", token("?."), false, false),
            "binary_expression" | "augmented_assignment_expression" => {
                for (op, key) in [
                    ("??", "fallback"),
                    ("??=", "fallback"),
                    ("||", "falsy-fallback"),
                    ("||=", "falsy-fallback"),
                    ("==", "loose-equality"),
                    ("!=", "loose-equality"),
                    ("===", "strict-equality"),
                    ("!==", "strict-equality"),
                ] {
                    emit(key, token(op), false, false);
                }
            }
            "for_in_statement" => {
                emit("keys", token("in"), false, true);
                emit("iterate-values", token("of"), false, true);
            }
            "spread_element" => {
                let key = match n.parent().map(|p| p.kind()) {
                    Some("arguments") => "spread-arguments",
                    Some("object") => "spread-properties",
                    _ => "spread-items",
                };
                emit(key, token("..."), false, false);
            }
            "rest_pattern" => {
                let parent = n.parent().map(|p| p.kind());
                let key = match parent {
                    Some("array_pattern") => "rest-items",
                    Some("object_pattern") => "rest-properties",
                    _ => "rest-arguments",
                };
                let pin = crate::syntax::named_children(n)
                    .into_iter()
                    .find(|c| c.kind() == "identifier")
                    .map(|c| format!("var:{}", text(c)));
                if let Some(t) = token("...") {
                    add(key, t, n, false, true, pin);
                }
            }
            "as_expression" if !javascript => {
                let key = if token("const").is_some() {
                    "as-const"
                } else {
                    "type-assertion"
                };
                emit(key, token("as"), false, false);
            }
            "type_assertion" if !javascript => emit("type-assertion", token("<"), false, false),
            "satisfies_expression" if !javascript => {
                emit("satisfies", token("satisfies"), false, false)
            }
            "non_null_expression" if !javascript => emit("non-null", token("!"), false, false),
            "import_statement" | "import_specifier" if !javascript => {
                emit("type-import", token("type"), true, true)
            }
            "accessibility_modifier" if !javascript && text(n) == "private" => {
                if let Some(parent) = n.parent() {
                    add("private", n, parent, true, true, None);
                }
            }
            "public_field_definition"
            | "property_signature"
            | "required_parameter"
            | "optional_parameter"
                if !javascript =>
            {
                emit("readonly", token("readonly"), true, true)
            }
            "private_property_identifier" => {
                if let Some(parent) = n.parent() {
                    let decl = parent
                        .child_by_field_name("name")
                        .is_some_and(|name| name.id() == n.id());
                    add("runtime-private", n, parent, decl, decl, None);
                }
            }
            _ => {}
        },
        Language::Rust => match kind {
            "reference_expression" | "reference_type" | "self_parameter" => {
                let key = if token("raw").is_some() {
                    "raw-borrow"
                } else if children(n).iter().any(|c| c.kind() == "mutable_specifier") {
                    "borrow-mut"
                } else {
                    "borrow"
                };
                let t = if key == "borrow-mut" {
                    children(n)
                        .into_iter()
                        .find(|c| c.kind() == "mutable_specifier")
                } else {
                    token("&")
                };
                emit(key, t, false, kind != "reference_expression");
            }
            "mutable_specifier"
                if n.parent().is_some_and(|p| {
                    !matches!(
                        p.kind(),
                        "reference_expression" | "reference_type" | "pointer_type"
                    )
                }) =>
            {
                emit("mut", Some(n), false, true)
            }
            "unary_expression" => emit("deref", token("*"), false, false),
            "try_expression" => emit("try", token("?"), false, false),
            "let_declaration" => emit("let-else", token("else"), false, true),
            "match_pattern" => {
                if let Some(t) = token("if")
                    && let Some((owner, pin)) = rust_match_case(n)
                {
                    add("match-guard", t, owner, false, true, Some(pin));
                }
            }
            "captured_pattern" | "or_pattern" | "remaining_field_pattern" | "_" => {
                if let Some((owner, pin)) = rust_pattern_owner(n) {
                    let site = match kind {
                        "captured_pattern" => token("@").map(|t| ("pattern-bind", t)),
                        "or_pattern" => token("|").map(|t| {
                            let key = if t.start_byte() == n.start_byte() { "pattern-leading-bar" } else { "pattern-alternative" };
                            (key, t)
                        }),
                        "remaining_field_pattern" => Some(("pattern-rest", n)),
                        _ => Some(("pattern-wildcard", n)),
                    };
                    if let Some((key, token)) = site {
                        add(key, token, owner, false, pin.is_some(), pin);
                    }
                }
            }
            "range_expression" | "range_pattern" => {
                let split_inclusive = n.parent().is_some_and(|parent| {
                    parent.child_by_field_name("left") == Some(n)
                        && rust_inclusive_range_to(parent).is_some()
                });
                if !split_inclusive {
                    emit("range-open", token(".."), false, false);
                }
                emit("range-inclusive", token("..="), false, false);
            }
            "assignment_expression" => {
                if let Some((equal, _)) = rust_inclusive_range_to(n) {
                    emit("range-inclusive", Some(equal), false, false);
                }
            }
            "macro_invocation" => emit("macro", token("!"), false, false),
            "macro_definition" => emit("macro-definition", token("macro_rules!"), true, true),
            "trait_item" => emit("trait-contract", token("trait"), true, true),
            "type_parameters" | "where_clause" | "trait_bounds" | "abstract_type" | "dynamic_type" => {
                let owner = std::iter::successors(n.parent(), Node::parent)
                    .take_while(|parent| parent.kind() != "block")
                    .find(|parent| matches!(parent.kind(), "function_item" | "function_signature_item" | "struct_item"
                        | "enum_item" | "trait_item" | "impl_item" | "field_declaration" | "type_item" | "associated_type"));
                let is_return = owner.and_then(|parent| parent.child_by_field_name("return_type"))
                    .is_some_and(|ty| ty.start_byte() <= n.start_byte() && ty.end_byte() >= n.end_byte());
                let key = match kind {
                    "type_parameters" => "type-parameters",
                    "where_clause" => "where-constraints",
                    "trait_bounds" => "trait-constraints",
                    "abstract_type" if is_return => "opaque-return",
                    "abstract_type" => "generic-parameter-type",
                    _ => "trait-object",
                };
                add(key, n, owner.unwrap_or(n), owner.is_some(), true, None);
            }
            "attribute_item" => {
                if let Some(attribute) = n.named_child(0)
                    && let Some(name) = attribute.named_child(0)
                    && let Some(owner) = std::iter::successors(n.next_named_sibling(), Node::next_named_sibling)
                        .find(|node| node.kind() != "attribute_item" && !node.is_extra())
                {
                    match text(name) {
                        "cfg" => add("cfg", n, owner, true, true, None),
                        "derive" => add("derive-traits", n, owner, true, true, None),
                        _ => {}
                    }
                }
            }
            "generic_function" => emit("explicit-type-args", token("::"), false, false),
            "lifetime" => {
                let mut parent = n.parent();
                while let Some(p) = parent {
                    if matches!(
                        p.kind(),
                        "function_item"
                            | "function_signature_item"
                            | "struct_item"
                            | "enum_item"
                            | "trait_item"
                            | "impl_item"
                    ) {
                        add("lifetime", n, p, true, true, None);
                        break;
                    }
                    if p.kind() == "block" {
                        break;
                    }
                    parent = p.parent();
                }
            }
            "closure_expression" => emit("move", token("move"), false, false),
            _ => {}
        },
    }
}
