//! Explain hexadecimal byte escapes in ordinary and byte strings.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Vec<(Span, String)> {
    if node.kind() != "string_literal" || node.has_error() { return Vec::new(); }
    let literal = src.text(node);
    let (byte_string, start) = if literal.starts_with("b\"") { (true, 2) } else if literal.starts_with('"') { (false, 1) } else { return Vec::new(); };
    if !literal.ends_with('"') { return Vec::new(); }
    let bytes = literal.as_bytes();
    let mut i = start;
    let mut result = Vec::new();
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' { i += 1; continue; }
        if bytes.get(i + 1) == Some(&b'x') && i + 3 < bytes.len() - 1 && bytes[i+2].is_ascii_hexdigit() && bytes[i+3].is_ascii_hexdigit() {
            let digits = &literal[i + 2..i + 4];
            let value = u8::from_str_radix(digits, 16).unwrap();
            if byte_string || value <= 0x7f {
                let meaning = if byte_string { "In a byte string this contributes one byte, which may be any value from 00 to FF; the result need not be valid UTF-8." } else { "In an ordinary text string this contributes the corresponding ASCII character, encoded as one UTF-8 byte. Rust restricts this escape here to 00 through 7F; use a Unicode escape for higher characters." };
                result.push((src.index.span(src.file, node.start_byte() + i, node.start_byte() + i + 4), format!("`\\x{digits}` is an escape with exactly two hexadecimal digits: it represents byte value 0x{value:02X} ({value} in decimal), not the four literal characters backslash, x, and the digits. {meaning} Raw strings do not interpret escapes; an escaped backslash `\\\\` does not start a hexadecimal escape.")));
            }
            i += 4;
        } else { i += 2; }
    }
    result
}
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Vec<(Span, ReadingContent)> {
    describe(src, node).into_iter().map(|(span, text)| (span, ReadingContent::Detail { label: "Hex escape in a string".into(), text, definition: None, borrow: false })).collect()
}
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == "token_tree" { continue; }
        result.extend(describe(src, node).into_iter().map(|(span, text)| ContractItem { group: "Hex escape in a string".into(), text, span, type_info: None }));
        pending.extend(named_children(node));
    }
    result
}
