//! Explain raw C string literals written without hash delimiters.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "raw_string_literal" || node.has_error() || !src.text(node).starts_with("cr\"") { return None; }
    let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
    Some((prefix, "The `c` prefix makes a C string literal and `r` makes its contents raw: backslashes do not start escapes, so `\\n` is a backslash followed by n, not a newline. This no-hash form ends at the next double quote; hashes can be added to delimit content containing quotes. The compiler encodes the contents as UTF-8, rejects an embedded NUL byte, and appends a terminating NUL byte. The value has type `&'static core::ffi::CStr`, a borrowed C string in static storage, not an owned Rust String or a C pointer by itself; `as_ptr()` obtains a pointer when needed.".into()))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Raw C string literal".into(), text, definition: None, borrow: false }))
}

/// Const and static initializers may not have executable flow nodes.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut found = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = describe(src, part) {
            found.push(ContractItem { group: "Raw C string literal".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    found
}
