//! Explain ordinary assignment, including its unit result.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" || node.has_error() { return None; }
    let mut place = node.child_by_field_name("left")?;
    while place.kind() == "parenthesized_expression" {
        let parts: Vec<_> = named_children(place).into_iter().filter(|part| !part.is_extra()).collect();
        let [inner] = parts.as_slice() else { return None; };
        place = *inner;
    }
    let valid = matches!(place.kind(), "identifier" | "field_expression" | "index_expression")
        || (place.kind() == "unary_expression" && children(place).iter().any(|part| part.kind() == "*"));
    if !valid || src.text(place) == "_" { return None; }
    let value = node.child_by_field_name("right")?;
    let equals = children(node).into_iter().find(|part| part.kind() == "=")?;
    Some((equals, ReadingContent::Detail {
        label: "Assignment and unit result".into(),
        text: format!("`{} = {}` evaluates the right-hand expression first, then writes its value to the existing destination `{}`. If the destination was initialized, its previous value is dropped as required before replacement. Whether the value is copied or moved depends on its type; the destination must be a valid writable place and the types must be compatible. The assignment expression itself evaluates to `()` (unit), the value representing no meaningful result; it does not evaluate to the assigned value. This is assignment to an existing place, not a new `let` binding.", src.norm(place), src.norm(value), src.norm(place)),
        definition: None, borrow: false,
    }))
}
