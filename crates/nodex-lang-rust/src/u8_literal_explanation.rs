//! Explain integer literals explicitly suffixed with Rust's built-in `u8` type.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn details(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let digits = literal.strip_suffix("u8")?;
    if digits.is_empty() { return None; }
    let suffix = src.index.span(src.file, node.end_byte() - 2, node.end_byte());
    Some((suffix, format!("{} is an integer literal. The suffix u8 explicitly gives this literal Rust's built-in unsigned 8-bit integer type; it is not a variable name or a runtime conversion. A u8 can represent whole numbers from 0 through 255. The portion before the suffix gives the number (it can use separators or a radix prefix). The compiler normally diagnoses a literal outside the type's range via the overflowing_literals lint, which can be configured. With an explicit suffix the literal's type is written rather than chosen solely by inference.", literal)))
}

/// Emit a source-linked explanation on the literal in an executable flow.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (suffix, text) = details(src, node)?;
    Some((suffix, ReadingContent::Detail { label: "Integer literal with u8 suffix".into(), text, definition: None, borrow: false }))
}

/// Const and static initializers do not necessarily have their own flow nodes;
/// expose their written literals in the owning symbol's visible contract.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "const_item" | "static_item" | "enum_variant") { return Vec::new(); }
    let Some(value) = node.child_by_field_name("value") else { return Vec::new(); };
    let mut pending = vec![value];
    let mut items = Vec::new();
    while let Some(part) = pending.pop() {
        if part != value && matches!(part.kind(), "token_tree" | "closure_expression" | "function_item" | "async_block") { continue; }
        if let Some((span, text)) = details(src, part) {
            items.push(ContractItem { group: "Integer literal with u8 suffix".into(), text, span, type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    items
}
