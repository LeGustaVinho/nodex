//! Explain an integer literal's `i32` suffix without inferring its type from context.
use nodex_ir::{Span, reading::{ContractItem, ReadingContent}};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<(Span, String)> {
    if node.kind() != "integer_literal" || node.has_error() { return None; }
    let literal = src.text(node);
    let number = literal.strip_suffix("i32")?;
    if number.is_empty() { return None; }
    let start = node.end_byte().checked_sub(3)?;
    let suffix = src.index.span(src.file, start, node.end_byte());
    Some((suffix, format!("In {literal}, {number} is the numeric portion and i32 is a type suffix. The suffix makes this literal a signed 32-bit integer, whose range is -2,147,483,648 through 2,147,483,647. It specifies the literal's type directly rather than performing a runtime cast. An underscore before the suffix is an optional separator, not part of the numeric value. The compiler checks that the literal and its use are valid in context.")))
}

/// Details on literals in executable flow; the suffix gets its own highlight.
pub(crate) fn reading(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    let (span, text) = describe(src, node)?;
    Some((span, ReadingContent::Detail { label: "Integer literal with i32 suffix".into(), text, definition: None, borrow: false }))
}

/// Constant and static initializers do not have ordinary function flow nodes.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "const_item" | "static_item") { return Vec::new(); }
    let Some(value) = declaration.child_by_field_name("value") else { return Vec::new() };
    let mut todo = vec![value];
    let mut facts = Vec::new();
    while let Some(node) = todo.pop() {
        if node.kind() == "token_tree" { continue; }
        if let Some((span, text)) = describe(src, node) {
            facts.push(ContractItem { group: "Integer literal with i32 suffix".into(), text, span, type_info: None });
        }
        todo.extend(named_children(node).into_iter().rev());
    }
    facts
}
