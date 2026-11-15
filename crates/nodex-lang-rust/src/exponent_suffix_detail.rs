//! Explain decimal scientific notation with an explicit f64 suffix.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "float_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let number = literal.strip_suffix("f64")?;
    let exponent_index = number.find(['e', 'E'])?;
    let mantissa = &number[..exponent_index];
    let exponent = number[exponent_index + 1..].trim_end_matches('_');
    if mantissa.is_empty() || exponent.is_empty() || !mantissa.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '_')) { return None; }
    let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if !digits.chars().any(|c| c.is_ascii_digit()) || !digits.chars().all(|c| c.is_ascii_digit() || c == '_') { return None; }
    let span = src.index.span(src.file, node.start_byte() + exponent_index, node.start_byte() + exponent_index + 1 + exponent.len());
    Some((span, format!("`{literal}` is a floating-point literal in scientific notation: `{mantissa}` is multiplied by 10 raised to the exponent `{exponent}`. A negative exponent moves the decimal point left; a positive exponent moves it right. The `f64` suffix explicitly selects Rust's 64-bit floating-point type; it is part of the literal, not a runtime cast. Underscores act as digit separators. Decimal values may require rounding to binary floating-point, so the stored result need not be exact. Rust checks whether the complete literal is valid.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Scientific notation with f64 suffix".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "token_tree" | "macro_invocation") { continue; }
        if let Some((span, text)) = describe(src, part) {
            items.push(ContractItem { group: "Scientific notation with f64 suffix".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
