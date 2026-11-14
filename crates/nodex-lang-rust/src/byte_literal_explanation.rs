//! Explain Rust single-byte literals, without confusing them with chars or byte strings.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Decode only byte-literal forms whose spelling is valid without expansion.
fn byte_value(src: Src<'_>, node: Node<'_>) -> Option<(Span, u8)> {
    if node.kind() != "char_literal" || node.has_error() { return None; }
    let text = src.text(node);
    let inner = text.strip_prefix("b'")?.strip_suffix('\'')?;
    let bytes = inner.as_bytes();
    let value = match bytes {
        [byte] if byte.is_ascii_graphic() && !matches!(*byte, b'\'' | b'\\') => *byte,
        [b' '] => b' ',
        [b'\\', b'n'] => b'\n',
        [b'\\', b'r'] => b'\r',
        [b'\\', b't'] => b'\t',
        [b'\\', b'0'] => 0,
        [b'\\', b'\\'] => b'\\',
        [b'\\', b'\''] => b'\'',
        [b'\\', b'"'] => b'"',
        [b'\\', b'x', a, b] if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() => {
            u8::from_str_radix(&inner[2..], 16).ok()?
        }
        _ => return None,
    };
    Some((src.index.span(src.file, node.start_byte(), node.start_byte() + 1), value))
}

fn text(src: Src<'_>, node: Node<'_>, value: u8) -> String {
    format!("{} is a byte literal: b before the single quotes selects one u8 byte, here with value {}. It is not a string and is different from Rust's ordinary char literal, whose type represents one Unicode scalar value. A directly written character in a byte literal must be ASCII; a byte escape such as \\xFF can represent a byte above 127. This literal already has type u8; no runtime character-to-byte conversion is performed.", src.text(node), value)
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, value) = byte_value(src, node)?;
    Some((span, ReadingContent::Detail { label: "Single-byte literal".into(), text: text(src, node, value), definition: None, borrow: false }))
}

/// Constants and statics expose initializer details through their symbol contract.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, value)) = byte_value(src, part) {
            result.push(ContractItem { group: "Single-byte literal".into(), text: text(src, part, value), span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
