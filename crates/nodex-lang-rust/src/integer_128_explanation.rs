//! Explain Rust's 128-bit integer types in declaration signatures.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    // Only inspect syntax belonging to this symbol's declaration. In particular, a
    // function body can contain unrelated local declarations or nested items.
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type" | "let_declaration") {
        return Vec::new();
    }
    let body = node.child_by_field_name("body");
    // A local declaration's initializer is an expression, not part of its
    // annotated type, and can contain unrelated nested declarations.
    let mut pending: Vec<_> = if node.kind() == "let_declaration" {
        node.child_by_field_name("type").into_iter().collect()
    } else { vec![node] };
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body { continue; }
        if part.kind() == "primitive_type" {
            let description = match src.text(part) {
                "u128" => Some("u128 is an unsigned 128-bit integer: it stores whole numbers from 0 through 2^128 - 1, inclusive."),
                "i128" => Some("i128 is a signed 128-bit integer: it stores whole numbers from -2^127 through 2^127 - 1, inclusive."),
                _ => None,
            };
            if let Some(text) = description {
                result.push(ContractItem { group: "Integer type".into(), text: text.into(), span: src.span(part), type_info: None });
            }
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}

/// Convert explicit local type annotations into source-linked flow details.
pub(crate) fn local(src: Src<'_>, node: Node<'_>) -> Vec<(nodex_ir::Span, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    explain(src, node).into_iter().map(|item| {
        // The general explicit-type detail uses the complete type span. Attach
        // a top-level primitive to the colon so the two details both survive.
        let span = if node.child_by_field_name("type").is_some_and(|ty| src.span(ty) == item.span) {
            children(node).into_iter().find(|part| part.kind() == ":").map_or(item.span.clone(), |part| src.span(part))
        } else { item.span.clone() };
        (span, ReadingContent::Detail {
        label: item.group,
        text: item.text,
        definition: None,
        borrow: false,
    }) }).collect()
}
