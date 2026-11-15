//! Explain separated binary integer literals in flows and constant declarations.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let rest = literal.strip_prefix("0b")?;
    let end = rest.bytes().take_while(|b| matches!(b, b'0' | b'1' | b'_')).count();
    let digits = &rest[..end];
    if !digits.contains('_') || !digits.bytes().any(|b| matches!(b, b'0' | b'1')) { return None; }
    let suffix = &rest[end..];
    if !suffix.is_empty() && !matches!(suffix, "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize") { return None; }
    let normalized: String = digits.chars().filter(|c| *c != '_').collect();
    let decimal = u128::from_str_radix(&normalized, 2).ok().map(|n| format!(" The digits {normalized} represent {n} in decimal.")).unwrap_or_default();
    let span = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
    Some((span, format!("In {literal}, `0b` introduces a base-two integer: the digits are 0 and 1, and their place values are powers of two. Underscores are visual separators and do not change the value.{decimal} A type suffix, when present, specifies the integer type; otherwise Rust infers it with integer fallback rules. The compiler checks whether the literal fits its type. This is a numeric value, not a string or a runtime conversion.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Binary integer with separators".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut found = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = describe(src, part) {
            found.push(ContractItem { group: "Binary integer with separators".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    found
}
