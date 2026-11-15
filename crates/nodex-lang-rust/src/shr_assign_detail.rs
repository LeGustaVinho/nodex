//! Explain right-shift assignment without guessing operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != ">>=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Shift right and assign".into(),
        text: format!("`{} >>= {}` updates the assignable place on the left using the right-shift assignment operation and the value on the right. For primitive integers this shifts bits to the right by the specified amount; signed integers use arithmetic right shift, while unsigned integers shift in zero bits. Invalid shift counts are overflow conditions. For other types the `ShrAssign` implementation determines behavior, so syntax alone does not establish the result. The place must be writable and the whole assignment expression produces `()` (unit). Operand evaluation order depends on types known before monomorphization; when both are known primitive, the right side is evaluated first. Rewriting this as a separate shift and assignment can change evaluation or borrowing behavior.", src.norm(left), src.norm(right)),
        definition: None, borrow: false,
    }))
}
