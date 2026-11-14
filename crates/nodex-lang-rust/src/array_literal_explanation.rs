//! Explain a list-form array value without treating an assignment target as construction.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "array_expression" || node.has_error() || node.child_by_field_name("length").is_some() { return None; }
    // Expressions on the left of destructuring assignment are destinations, not array values.
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && left.end_byte() >= node.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item" | "closure_expression") { break; }
    }
    let bracket = children(node).into_iter().find(|part| part.kind() == "[")?;
    let count = named_children(node).into_iter().filter(|part| !part.is_extra()).count();
    Some((bracket, ReadingContent::Detail {
        label: "Create a fixed-size array".into(),
        text: format!("The brackets list {count} element expression(s) in order and create an array value. Rust evaluates the elements left to right, proceeding to the next only when the previous one completes normally. All elements must have one common type after any allowed coercions; Rust can infer that type from the elements and surrounding context. The written list fixes the length at {count}, and the length is part of the array's type. Positions start at index 0."),
        definition: None,
        borrow: false,
    }))
}
