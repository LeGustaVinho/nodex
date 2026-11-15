//! Explain digit separators in decimal integer literals.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let written = src.text(node);
    if written.starts_with("0x") || written.starts_with("0o") || written.starts_with("0b") { return None; }
    let end = written.bytes().take_while(|b| b.is_ascii_digit() || *b == b'_').count();
    let digits = &written[..end];
    if !digits.as_bytes().windows(2).any(|w| w[0] == b'_' && w[1].is_ascii_digit()) { return None; }
    let normalized: String = digits.chars().filter(|c| *c != '_').collect();
    if normalized.is_empty() { return None; }
    let first = digits.find('_')?;
    let span = src.index.span(src.file, node.start_byte() + first, node.start_byte() + first + 1);
    Some((span, format!("In {written}, the underscores in the decimal digits are visual separators for readability. They are ignored when determining the numeric value: {digits} denotes the same decimal integer as {normalized}. They do not create separate numbers or perform an operation. A written suffix specifies a type; without one Rust uses type inference and fallback, and the compiler checks whether the value fits its type.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Decimal digit separators".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new(); };
    let mut todo = vec![value];
    let mut result = Vec::new();
    while let Some(node) = todo.pop() {
        if node != value && matches!(node.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = describe(src, node) { result.push(ContractItem { group: "Decimal digit separators".into(), text, span, type_info: None }); }
        todo.extend(named_children(node).into_iter().rev());
    }
    result
}
