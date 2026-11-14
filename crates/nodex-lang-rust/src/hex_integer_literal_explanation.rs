//! Explain hexadecimal integer notation independently of any type suffix.
use nodex_ir::{reading::{ContractItem, ReadingContent}, Span};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn details(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let written = src.text(node);
    let tail = written.strip_prefix("0x")?;
    let end = tail.bytes().take_while(|b| b.is_ascii_hexdigit() || *b == b'_').count();
    let digits = &tail[..end];
    let normalized: String = digits.chars().filter(|c| *c != '_').collect();
    if normalized.is_empty() { return None; }
    let numeric = u128::from_str_radix(&normalized, 16).ok()
        .map(|value| format!(" The hexadecimal digits {normalized} represent {value} in decimal."))
        .unwrap_or_default();
    let span = src.index.span(src.file, node.start_byte(), node.start_byte() + 2);
    Some((span, format!("In {written}, the prefix `0x` means the integer digits use base 16. Each digit is 0–9 or A–F (case-insensitive), with A through F representing 10 through 15. Underscores may separate digits or the suffix for readability and do not change the numeric value.{numeric} A type suffix, when present, specifies the literal's type; the compiler checks whether the literal is valid for that type. This notation is still an integer value, not a string or a runtime conversion.")))
}

pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = details(src, node)?;
    Some((span, ReadingContent::Detail { label: "Hexadecimal integer literal".into(), text, definition: None, borrow: false }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = details(src, part) {
            items.push(ContractItem { group: "Hexadecimal integer literal".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
