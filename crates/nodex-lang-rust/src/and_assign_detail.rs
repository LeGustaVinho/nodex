//! Explain bitwise-and compound assignment without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "&=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "AND and assign".into(),
        text: format!("`{} &= {}` updates the mutable place on the left using the AND-assignment operation and the value on the right. For built-in integer operands, AND combines corresponding bits, producing a 1 only where both bits are 1; for booleans it computes AND with both operands evaluated, without short-circuiting. For other types the `BitAndAssign` implementation determines the behavior, so the written syntax alone does not prove the result. The left side must be an assignable place, and the whole assignment expression produces `()` (unit). Operand evaluation order depends on the types known before monomorphization; when both are known primitive, the right side is evaluated first. Rewriting this as a separate AND and assignment need not preserve evaluation and borrowing behavior.", src.norm(left), src.norm(right)),
        definition: None, borrow: false,
    }))
}
