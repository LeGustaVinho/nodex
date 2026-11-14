//! Explain the comma in a one-element tuple expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_expression" || node.has_error() { return None; }
    // A tuple-shaped left-hand side is a destructuring assignment target,
    // not construction of a new tuple value.
    let mut part = node;
    while let Some(parent) = part.parent() {
        if parent.kind() == "assignment_expression" {
            if parent.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && node.end_byte() <= left.end_byte()) { return None; }
            break;
        }
        if !matches!(parent.kind(), "parenthesized_expression" | "tuple_expression" | "array_expression" | "field_initializer" | "field_initializer_list") { break; }
        part = parent;
    }
    let elements: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [element] = elements.as_slice() else { return None; };
    let comma = children(node).into_iter().find(|part| part.kind() == "," && !part.is_missing())?;
    Some((comma, ReadingContent::Detail {
        label: "Create a one-element tuple".into(),
        text: format!("Evaluate {} and put its result in position 0 of a new one-element tuple. The trailing comma is essential: ({},) is a tuple, while ({}) without the comma only groups the expression and produces its value directly. Access the tuple's element with `.0`. Whether using the element expression copies, moves, or borrows a value depends on that expression and its type.", src.norm(*element), src.norm(*element), src.norm(*element)),
        definition: None,
        borrow: false,
    }))
}
