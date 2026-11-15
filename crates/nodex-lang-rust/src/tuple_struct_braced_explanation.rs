//! Explain braced construction using numeric fields, as in `Pair { 0: 1, 1: 2 }`.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.kind() != "struct_expression" || node.has_error() { return None; }
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && left.end_byte() >= node.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    let fields = named_children(body);
    if fields.is_empty() { return None; }
    let mut descriptions = Vec::new();
    for field in fields {
        if field.kind() != "field_initializer" { return None; }
        let index = field.child_by_field_name("field")?;
        let index_text = src.text(index);
        if index_text.is_empty() || !index_text.chars().all(|c| c.is_ascii_digit()) { return None; }
        descriptions.push(format!("position {} receives {}", index_text, src.norm(field.child_by_field_name("value")?)));
    }
    Some(ReadingContent::Detail {
        label: "Construct with positional fields".into(),
        text: format!("The braces after {} provide fields by their zero-based position: {}. If {} resolves to a tuple struct (or tuple-like variant), this constructs that value using its positional fields; for a tuple struct, the more familiar `Type(value, ...)` form may also be available. The numeric labels select fields, not array indexes or new variable names. Field visibility and the declaration determine whether this construction is valid; the written name alone does not establish what it resolves to.", src.norm(name), descriptions.join("; "), src.norm(name)),
        borrow: false, definition: None,
    })
}
