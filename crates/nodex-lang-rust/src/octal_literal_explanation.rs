//! Explain the radix prefix on Rust octal integer literals.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let remainder = literal.strip_prefix("0o")?;
    let end = remainder.char_indices().find(|(_, ch)| !matches!(ch, '0'..='7' | '_')).map_or(remainder.len(), |(index, _)| index);
    let digits = &remainder[..end];
    if !digits.chars().any(|ch| matches!(ch, '0'..='7')) { return None; }
    let suffix = &remainder[end..];
    if !suffix.is_empty() && !matches!(suffix, "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize") { return None; }
    let decimal = digits.chars().filter(|ch| *ch != '_').try_fold(0u128, |value, ch| value.checked_mul(8)?.checked_add(u128::from(ch.to_digit(8)?)));
    let number = decimal.map(|value| format!(" Its digit portion has decimal value {value}.")).unwrap_or_default();
    let prefix = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
    Some((prefix, format!("In {literal}, the `0o` prefix means that the digits use base eight (octal). Each digit must be 0 through 7; place values are powers of eight, and underscores are visual separators rather than digits.{number} A type suffix, when written, specifies the integer type; otherwise the type is chosen by inference and fallback rules. The compiler checks that the value is valid for that type. The prefix describes a number; it does not itself apply file permissions.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Octal integer literal".into(), text, definition: None, borrow: false }))
}

/// Const/static initializers and discriminants may have no executable flow node.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut found = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = describe(src, part) {
            found.push(ContractItem { group: "Octal integer literal".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    found
}
