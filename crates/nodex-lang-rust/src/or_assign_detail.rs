//! Explain bitwise OR assignment without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "|=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "OR and assign".into(),
        text: format!("`{} |= {}` updates the place on the left using the value on the right; this assignment expression produces `()`. For integers, it stores the bitwise OR: each result bit is 1 when either corresponding input bit is 1. For booleans, it stores the logical OR of the two values. Unlike `||`, this operator is not short-circuiting: the right operand is evaluated. Other types can implement `std::ops::BitOrAssign` and define their own behavior. The left side must be a suitable mutable place, and type checking selects the operation; the syntax alone does not establish the operand types or implementation.", src.norm(left), src.norm(right)),
        definition: None, borrow: false,
    }))
}
