//! Explain `<<=` without assuming the types or implementation.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "<<=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Shift left and assign".into(),
        text: format!("`{} <<= {}` updates the mutable place on the left using a shift-assignment operation with the right-hand value. For primitive integer operands this shifts bits to the left by the specified count and stores the result; shift-count overflow rules apply. For other types, the selected `ShlAssign` implementation determines the behavior, so the syntax alone does not prove a bit shift or its result. The left side must be an assignable place, and the whole assignment expression produces `()` (unit). Operand evaluation order depends on types known before monomorphization; when both are known primitive, the right side is evaluated first. Rewriting as a separate shift and assignment can change evaluation or borrowing behavior.", src.norm(left), src.norm(right)),
        borrow: false, definition: None,
    }))
}
