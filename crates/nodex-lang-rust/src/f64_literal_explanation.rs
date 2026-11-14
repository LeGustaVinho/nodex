//! Explain a floating-point literal explicitly suffixed with `f64`.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "float_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let number = literal.strip_suffix("f64")?;
    if number.is_empty() { return None; }
    let suffix = src.index.span(src.file, node.end_byte().checked_sub(3)?, node.end_byte());
    Some((suffix, format!("In {literal}, {number} is the numeric portion and f64 is a type suffix: it explicitly makes the literal a 64-bit floating-point value rather than performing a runtime conversion. Rust's f64 uses IEEE 754 binary64. The compiler converts the decimal spelling to that format; some decimal values need rounding because they cannot be represented exactly (1.5 itself is exact). An underscore immediately before the suffix is an optional separator. The compiler checks the literal and its use in context.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Floating-point literal with f64 suffix".into(), text, definition: None, borrow: false }))
}

/// Constant and static initializers do not necessarily have executable-flow nodes.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) {
            items.push(ContractItem { group: "Floating-point literal with f64 suffix".into(), text, span, type_info: None });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
