//! Explain ordinary C string literals such as `c"hello"`.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "string_literal" || node.has_error() { return None; }
    let written = src.text(node);
    if !written.starts_with("c\"") { return None; }
    let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 1);
    Some((prefix, format!("The c prefix makes {written} a C string literal, not an ordinary Rust string. Its type is &'static core::ffi::CStr: a shared reference to a C-compatible byte sequence stored for the program's lifetime. Rust automatically appends a zero byte (NUL) terminator, and rejects a literal whose content contains an interior NUL. Ordinary text characters are encoded as UTF-8, while byte escapes can produce non-UTF-8 bytes, so the complete content is not guaranteed to be valid UTF-8. The value is a CStr reference, not a raw C pointer; code can obtain a pointer with as_ptr when the surrounding interface requires one.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "C string literal".into(), text, definition: None, borrow: false }))
}

/// Constant and static initializers may not have a function-flow node.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) {
            items.push(ContractItem { group: "C string literal".into(), text, span, type_info: None });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
