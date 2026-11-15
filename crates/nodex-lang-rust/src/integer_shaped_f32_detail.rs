//! Explain decimal integer-shaped tokens carrying an f32 suffix.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if !matches!(node.kind(), "integer_literal" | "float_literal") || node.has_error() { return None; }
    let literal = src.text(node);
    let digits = literal.strip_suffix("f32")?;
    if !digits.chars().any(|c| c.is_ascii_digit()) || !digits.chars().all(|c| c.is_ascii_digit() || c == '_') { return None; }
    let suffix = src.index.span(src.file, node.end_byte() - 3, node.end_byte());
    let example = if digits == "5" { " `5f32` represents exactly the floating-point value 5.0." } else { " Some values need rounding to fit binary32." };
    Some((suffix, format!("`{literal}` looks like a decimal integer because it has no decimal point or exponent, but the `f32` suffix makes it an `f32` floating-point literal expression. The suffix specifies the type directly; it is not a runtime cast or a separate identifier. Underscores in the digit portion are separators.{example} This explanation applies to a decimal spelling; a hexadecimal token such as `0x01_f32` has different lexical rules and its `f32` characters are hexadecimal digits.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Integer-shaped f32 literal".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "token_tree" | "macro_invocation") { continue; }
        if let Some((span, text)) = describe(src, part) { result.push(ContractItem { group: "Integer-shaped f32 literal".into(), text, span, type_info: None }); }
        pending.extend(named_children(part));
    }
    result
}
