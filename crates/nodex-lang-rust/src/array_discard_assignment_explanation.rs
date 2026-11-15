//! Explain a three-position destructuring assignment with a wildcard destination.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" || node.has_error() { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    if left.kind() != "array_expression" || left.child_by_field_name("length").is_some() { return None; }
    let parts: Vec<_> = named_children(left).into_iter().filter(|part| !part.is_extra()).collect();
    let [first, ignored, last] = parts.as_slice() else { return None; };
    if first.kind() != "identifier" || last.kind() != "identifier" || src.text(*ignored) != "_" || src.text(*first) == "_" || src.text(*last) == "_" { return None; }
    Some((*ignored, ReadingContent::Detail {
        label: "Discard an array assignment element".into(),
        text: format!("This is an assignment to existing destinations, not a `let` declaration. Evaluate {} first; then assign its first element to {} and its third element to {} in left-to-right destination order. `_` ignores the middle element: it is not a variable and receives no assignment. The named destinations must be valid assignable places, and values are copied or moved as their types and context require. If evaluation of the right side exits early, these destination assignments do not happen.", src.norm(right), src.norm(*first), src.norm(*last)),
        definition: None,
        borrow: false,
    }))
}
