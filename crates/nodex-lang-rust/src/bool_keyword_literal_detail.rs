//! Explain the two boolean literal keywords.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "boolean_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    if !matches!(literal, "true" | "false") { return None; }
    Some((src.span(node), format!("`{literal}` is a boolean literal keyword of type `bool`. Rust's boolean type has exactly two values, `true` and `false`; this literal directly supplies the {} value. It can be used as a value or as a pattern matching that exact value, depending on context. It is not a string or a variable name.", if literal == "true" { "true" } else { "false" })))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Boolean literal keyword".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) { result.push(ContractItem { group: "Boolean literal keyword".into(), text, span, type_info: None }); }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
