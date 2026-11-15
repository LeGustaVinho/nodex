//! Explain `_` as a standalone assignment destination.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" || node.has_error() { return None; }
    let left = node.child_by_field_name("left")?;
    if src.text(left) != "_" { return None; }
    let right = node.child_by_field_name("right")?;
    Some((left, ReadingContent::Detail {
        label: "Discard an assignment result".into(),
        text: format!("In `_ = {}`, `_` is a discard destination, not a variable. The right-hand expression is evaluated, including its effects, but its result is not bound to a name or stored in a destination. A temporary result is cleaned up according to its temporary scope. The assignment expression produces `()` (unit). In a pattern, `_` instead matches without binding; in a permitted type or generic argument position, it asks the compiler to infer a type or argument. A name such as `_value` is a separate binding.", src.norm(right)),
        definition: None, borrow: false,
    }))
}
