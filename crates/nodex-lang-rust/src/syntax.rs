//! Small helpers over tree-sitter-rust trees.

use tree_sitter::Node;

pub use nodex_lang_api::syntax::{Src, children, first_error};
pub(crate) use nodex_lang_api::syntax_sites::rust_inclusive_range_to as inclusive_range_to;

/// Rust comments (`//`, `/* */`, doc comments) are extras that may appear anywhere.
pub fn is_comment(kind: &str) -> bool {
    matches!(kind, "line_comment" | "block_comment")
}

/// Named children without comments.
pub fn named_children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).filter(|c| !is_comment(c.kind())).collect()
}

pub fn children_of_kind<'t>(node: Node<'t>, kind: &str) -> Vec<Node<'t>> {
    named_children(node).into_iter().filter(|c| c.kind() == kind).collect()
}

pub fn first_child_of_kind<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named_children(node).into_iter().find(|c| c.kind() == kind)
}

/// Item declarations: they never run as statements.
pub fn is_item(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "union_item"
            | "trait_item"
            | "impl_item"
            | "mod_item"
            | "use_declaration"
            | "extern_crate_declaration"
            | "type_item"
            | "const_item"
            | "static_item"
            | "macro_definition"
            | "foreign_mod_item"
            | "attribute_item"
            | "inner_attribute_item"
            | "empty_statement"
    )
}

/// Expression node kinds that can stand where a value is expected.
pub(crate) fn is_expression(kind: &str) -> bool {
    !matches!(kind, "expression_statement" | "let_declaration" | "label")
        && !is_item(kind)
        && !is_comment(kind)
}

pub(crate) fn block_statements<'tree>(block: Node<'tree>) -> impl DoubleEndedIterator<Item = Node<'tree>> {
    named_children(block).into_iter().filter(|node| !node.is_extra() && node.kind() != "label")
}

/// A final expression may be wrapped as a statement when it ends with a block.
pub(crate) fn statement_tail(statement: Node<'_>) -> Option<Node<'_>> {
    if statement.kind() == "expression_statement" {
        let mut cursor = statement.walk();
        if statement.children(&mut cursor).any(|token| token.kind() == ";") {
            return None;
        }
        return named_children(statement).into_iter().find(|node| !node.is_extra());
    }
    is_expression(statement.kind()).then_some(statement)
}

/// The wildcard token when it is the entire `let` pattern, possibly grouped.
pub(crate) fn let_wildcard(declaration: Node<'_>) -> Option<Node<'_>> {
    if declaration.kind() != "let_declaration" || declaration.child_by_field_name("alternative").is_some() {
        return None;
    }
    let mut pattern = declaration.child_by_field_name("pattern")?;
    while let Some(inner) = nodex_lang_api::blueprint_types::rust_grouped_pattern(pattern) {
        pattern = inner;
    }
    (pattern.kind() == "_" && !pattern.is_missing()).then_some(pattern)
}

/// A plain path in a wildcard initializer supplies no value to read or capture.
pub(crate) fn discarded_path(mut node: Node<'_>) -> bool {
    if !matches!(node.kind(), "identifier" | "self" | "scoped_identifier") {
        return false;
    }
    while let Some(parent) = node.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
        node = parent;
    }
    node.parent().is_some_and(|parent| let_wildcard(parent).is_some()
        && parent.child_by_field_name("value") == Some(node))
}

/// Removes redundant parentheses.
pub fn unparen(mut node: Node) -> Node {
    while node.kind() == "parenthesized_expression" {
        match named_children(node).into_iter().next() {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

/// A grouped Rust type is parsed as a one-element tuple without a comma.
pub fn unparen_type(mut node: Node) -> Node {
    while node.kind() == "tuple_type" {
        let parts: Vec<_> = named_children(node).into_iter().filter(|child| !child.is_extra()).collect();
        if parts.len() != 1 || children(node).iter().any(|child| child.kind() == ",") {
            break;
        }
        node = parts[0];
    }
    node
}

/// True when the statement ends with `;` (an expression statement whose value is discarded).
pub fn has_semicolon(src: Src, node: Node) -> bool {
    src.text(node).trim_end().ends_with(';')
}

/// Attributes written before `node` (`#[derive(Debug)]`, `#[test]`), as `(path, args)`.
pub fn attributes<'t>(src: Src, node: Node<'t>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        match p.kind() {
            "attribute_item" => {
                if let Some(attr) = first_child_of_kind(p, "attribute") {
                    let path = named_children(attr).into_iter().next().map(|n| src.norm(n)).unwrap_or_default();
                    let args = attr.child_by_field_name("arguments").map(|a| src.norm(a)).unwrap_or_default();
                    out.push((path, args));
                }
            }
            k if is_comment(k) => {}
            _ => break,
        }
        prev = p.prev_named_sibling();
    }
    out.reverse();
    out
}

/// Attributes as flavors: `#[derive(Debug, Clone)]` → `derive:Debug`, `derive:Clone`;
/// `#[test]` → `test`; `#[cfg(test)]` → `#cfg(test)`; others `attr:name`.
pub fn attribute_flavors(src: Src, node: Node) -> Vec<String> {
    let mut out = Vec::new();
    for (path, args) in attributes(src, node) {
        // `#[unsafe(no_mangle)]` (Rust 2024): an attribute the author vouches for.
        let (path, args) = match path.as_str() {
            "unsafe" => {
                out.push("unsafe-attribute".into());
                let inner = args.trim_matches(['(', ')']).trim();
                let name = inner.split(['(', '=']).next().unwrap_or(inner).trim().to_owned();
                (name, inner.get(inner.find('(').unwrap_or(inner.len())..).unwrap_or("").to_owned())
            }
            _ => (path, args),
        };
        match path.as_str() {
            "derive" => {
                for d in args.trim_matches(['(', ')']).split(',') {
                    let d = d.trim().rsplit("::").next().unwrap_or("").trim();
                    if !d.is_empty() {
                        out.push(format!("derive:{d}"));
                    }
                }
            }
            "test" | "tokio::test" => out.push("test".into()),
            "cfg" => out.push(format!("#cfg{args}")),
            "inline" | "must_use" | "deprecated" | "non_exhaustive" => out.push(path.clone()),
            "async_trait" | "async_trait::async_trait" => out.push("async-trait".into()),
            "tokio::main" => out.push("async-main".into()),
            "doc" | "allow" | "warn" | "deny" | "expect" | "rustfmt::skip" => {}
            _ => out.push(format!("attr:{path}")),
        }
    }
    out
}

/// The `///` (or `/** */`) documentation written before `node`, as plain text.
pub fn doc_comment(src: Src, node: Node) -> Option<String> {
    let mut lines = Vec::new();
    let mut prev = node.prev_sibling();
    while let Some(p) = prev {
        match p.kind() {
            "line_comment" => {
                let t = src.text(p);
                match t.strip_prefix("///") {
                    Some(rest) if !rest.starts_with('/') => lines.push(rest.trim().to_owned()),
                    _ => break,
                }
            }
            "block_comment" => {
                let t = src.text(p);
                match t.strip_prefix("/**") {
                    Some(rest) if !rest.starts_with('*') => {
                        let body = rest.trim_end_matches("*/");
                        for l in body.lines().rev() {
                            lines.push(l.trim().trim_start_matches('*').trim().to_owned());
                        }
                    }
                    _ => break,
                }
            }
            "attribute_item" => {}
            _ => break,
        }
        prev = p.prev_sibling();
    }
    lines.reverse();
    let text = lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// `pub`, `pub(crate)`, ... of an item.
pub fn visibility(src: Src, node: Node) -> Option<String> {
    first_child_of_kind(node, "visibility_modifier").map(|v| src.norm(v))
}

/// Keywords of a function: `async`, `const`, `unsafe`, `extern "C"`.
pub fn function_modifiers(src: Src, func: Node) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(m) = first_child_of_kind(func, "function_modifiers") {
        for c in children(m) {
            let t = src.text(c);
            match c.kind() {
                "extern_modifier" => out.push("extern".to_owned()),
                _ if matches!(t, "async" | "const" | "unsafe" | "default") => out.push(t.to_owned()),
                _ => {}
            }
        }
    }
    out
}

/// Strips generic arguments and paths: `Vec<Order>` → `Vec`, `crate::a::B` → `B`, `&mut T` → `T`.
pub fn bare_type_name(text: &str) -> String {
    let t = text.trim().trim_start_matches('&').trim_start_matches("mut ").trim_start_matches("dyn ").trim_start_matches("impl ");
    let t = t.split('<').next().unwrap_or(t);
    t.rsplit("::").next().unwrap_or(t).trim().to_owned()
}

pub(crate) fn field_projection(base: Node) -> Option<Node> {
    base.parent().filter(|field| {
        field.kind() == "field_expression" && field.child_by_field_name("value") == Some(base)
            && !field.parent().is_some_and(|call| call.kind() == "call_expression"
                && call.child_by_field_name("function") == Some(*field))
    })
}

/// Type names used by a type expression (`Option<Vec<Order>>` → `Option`, `Vec`, `Order`),
/// without primitives.
pub fn type_names(src: Src, ty: Node, out: &mut Vec<String>) {
    match ty.kind() {
        "type_identifier" => {
            let name = src.text(ty).to_owned();
            if !out.contains(&name) {
                out.push(name);
            }
        }
        "scoped_type_identifier" => {
            if let Some(n) = ty.child_by_field_name("name") {
                let name = src.text(n).to_owned();
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        "primitive_type" | "lifetime" => {}
        _ => {
            for c in named_children(ty) {
                type_names(src, c, out);
            }
        }
    }
}

/// `(name, type)` of each parameter; `self` receivers are skipped (see [`receiver`]).
pub fn parameters(src: Src, func: Node) -> Vec<(String, Option<String>)> {
    let Some(list) = func.child_by_field_name("parameters") else { return Vec::new() };
    let mut out = Vec::new();
    for p in named_children(list) {
        match p.kind() {
            "parameter" => {
                let name = p.child_by_field_name("pattern").map(|n| pattern_text(src, n)).unwrap_or_default();
                let ty = p.child_by_field_name("type").map(|t| src.norm(t));
                out.push((name, ty));
            }
            "variadic_parameter" => out.push(("...".into(), None)),
            // Closure parameters: bare patterns or `x: T`.
            "identifier" => out.push((src.text(p).to_owned(), None)),
            k if k.ends_with("pattern") => out.push((pattern_text(src, p), None)),
            _ => {}
        }
    }
    out
}

/// The `self` receiver of a method, including explicitly typed receivers.
pub fn receiver(src: Src, func: Node) -> Option<String> {
    let list = func.child_by_field_name("parameters")?;
    let s = named_children(list).into_iter().find(|parameter| {
        parameter.kind() == "self_parameter"
            || (parameter.kind() == "parameter"
                && parameter.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "self"))
    })?;
    let text = src.norm(s);
    Some(if text.contains("&mut") || text.contains("& mut") {
        "&mut self".into()
    } else if text.starts_with('&') {
        "&self".into()
    } else {
        "self".into()
    })
}

/// A pattern as written, without `mut`/`ref` noise for plain bindings.
fn pattern_text(src: Src, p: Node) -> String {
    match p.kind() {
        "mut_pattern" | "ref_pattern" => named_children(p).into_iter().last().map(|n| pattern_text(src, n)).unwrap_or_default(),
        _ => src.norm(p),
    }
}

/// Each name in a pattern and whether the binding itself may be reassigned.
pub fn pattern_bindings(src: Src, pattern: Node, mutable: bool, out: &mut Vec<(String, bool)>) {
    let name = match pattern.kind() {
        "identifier" if mutable || !src.text(pattern).chars().next().is_some_and(char::is_uppercase) => Some(pattern),
        "shorthand_field_identifier" => Some(pattern),
        "captured_pattern" => {
            let mut parts = named_children(pattern).into_iter();
            // The identifier to the left of `@` is always a binding.
            if let Some(binding) = parts.next().filter(|node| node.kind() == "identifier") {
                record_binding(src.text(binding), mutable, out);
            }
            if let Some(inner) = parts.next() {
                pattern_bindings(src, inner, false, out);
            }
            return;
        }
        "field_pattern" => {
            if let Some(inner) = pattern.child_by_field_name("pattern") {
                pattern_bindings(src, inner, false, out);
                return;
            }
            pattern.child_by_field_name("name")
        }
        "identifier" | "scoped_identifier" | "type_identifier" | "scoped_type_identifier"
        | "_literal_pattern" | "integer_literal" | "string_literal" | "char_literal"
        | "boolean_literal" | "float_literal" | "negative_literal" | "range_pattern" => None,
        kind => {
            let children = named_children(pattern);
            let skip = usize::from(matches!(kind, "tuple_struct_pattern" | "struct_pattern"));
            for child in children.into_iter().skip(skip).filter(|n| n.kind() != "mutable_specifier") {
                let mutable = match kind {
                    "mut_pattern" => true,
                    // A ref binding's access can be mutable while the binding itself is fixed.
                    "ref_pattern" if child.kind() == "mut_pattern" => {
                        if let Some(inner) = named_children(child).into_iter().last() {
                            pattern_bindings(src, inner, false, out);
                        }
                        continue;
                    }
                    _ => false,
                };
                pattern_bindings(src, child, mutable, out);
            }
            None
        }
    };
    if let Some(name) = name {
        let name = src.text(name);
        let field_mut = pattern.kind() == "field_pattern"
            && first_child_of_kind(pattern, "mutable_specifier").is_some()
            && !children(pattern).iter().any(|n| n.kind() == "ref");
        record_binding(name, mutable || field_mut, out);
    }
}

fn record_binding(name: &str, mutable: bool, out: &mut Vec<(String, bool)>) {
    if name != "_" && !out.iter().any(|(existing, _)| existing == name) {
        out.push((name.to_owned(), mutable));
    }
}

/// Last segment of a path expression: `std::mem::take` → `take`, `Vec::<T>::new` → `new`.
pub fn last_segment(text: &str) -> &str {
    let t = text.rsplit("::").next().unwrap_or(text);
    t.split('<').next().unwrap_or(t).trim()
}
