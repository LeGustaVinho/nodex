//! Explain hash-delimited raw C string literals without confusing raw byte strings.
use nodex_ir::{reading::{ContractItem, ReadingContent}, Span};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn details(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "raw_string_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let tail = written.strip_prefix("cr")?;
    let hashes = tail.bytes().take_while(|byte| *byte == b'#').count();
    if hashes == 0 || tail.as_bytes().get(hashes) != Some(&b'"') { return None; }
    let ending = format!("\"{}", "#".repeat(hashes));
    if !written.ends_with(&ending) || written.len() < 2 + hashes + 1 + ending.len() { return None; }
    let span = src.index.span(src.file, node.start_byte(), node.start_byte() + 2 + hashes);
    Some((span, format!("{} is a raw C string literal. The `c` prefix produces a C-compatible string and `r` means backslashes are literal: `\\n` is a backslash followed by n, not a newline escape. The {} hash mark(s) after `cr` form part of the opening delimiter; it closes at a double quote followed by the same number of hashes, allowing other quotes inside. The delimiters are not part of the value. Its type is `&'static CStr`: its UTF-8 content bytes are followed by a terminating zero byte supplied by the compiler, and its content must not contain a zero byte. It is a borrowed C string, not Rust's owned `String`.", written, hashes)))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = details(src, node)?;
    Some((span, ReadingContent::Detail { label: "Raw C string with hash delimiters".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = details(src, part) {
            items.push(ContractItem { group: "Raw C string with hash delimiters".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
