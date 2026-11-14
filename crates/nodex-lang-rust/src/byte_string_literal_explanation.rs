//! Explain ordinary Rust byte-string literals such as `b"hello"`.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "string_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    if !literal.starts_with("b\"") { return None; }
    let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 1);
    let example = if literal == "b\"hello\"" { " In this example the five bytes are 104, 101, 108, 108, and 111." } else { "" };
    Some((prefix, format!("The b prefix makes {literal} a byte-string literal, not an ordinary text string. Its type is &'static [u8; N]: a shared reference to a fixed-size array of N bytes. N counts the bytes produced after escape sequences are processed.{example} Escape sequences can encode byte values such as \\xFF, so the result is not guaranteed to be valid UTF-8. Literal source characters must be ASCII; use byte escapes for other byte values. It does not automatically add a zero terminator.")))
}

/// Explain literals in executable code where they can attach to a flow node.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Byte-string literal".into(), text, definition: None, borrow: false }))
}

/// Constants and statics expose their initializers on the symbol contract.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new() };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, part) {
            result.push(ContractItem { group: "Byte-string literal".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
