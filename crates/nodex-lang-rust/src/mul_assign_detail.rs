//! Explain multiplication compound assignment without guessing operand types.
//! Rust Reference: https://doc.rust-lang.org/reference/expressions/operator-expr.html#compound-assignment-expressions
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "*=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Multiply and assign".into(),
        text: format!("`{} *= {}` updates the mutable place on the left using the multiplication-assignment operation and the value on the right. For primitive numeric operands this applies the built-in multiplication; for other types the corresponding `MulAssign` implementation determines the behavior, so the written syntax alone does not prove the result. The left side must be an assignable place, and the whole assignment expression produces `()` (unit). Operand evaluation order depends on the types known before monomorphization; when both are known primitive, the right side is evaluated first. Do not assume that rewriting it as a separate multiplication and assignment preserves all evaluation and borrowing behavior.", src.norm(left), src.norm(right)),
        definition: None,
        borrow: false,
    }))
}
