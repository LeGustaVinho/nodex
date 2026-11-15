//! Explain the addition-assignment operator without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "+=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Add and assign".into(),
        text: format!("`{} += {}` updates the place on the left using the value on the right; it is an assignment, and the expression itself produces the unit value `()`. For ordinary numeric values this adds the right value to the current left value and stores the result, subject to the numeric type and applicable overflow behavior. Rust also allows types to define `+=` through `std::ops::AddAssign`, so the selected operation and right-hand type depend on the operands and type checking. The left side must be a suitable mutable place. The syntax alone does not establish the operand types or which implementation applies.", src.norm(left), src.norm(right)),
        definition: None, borrow: false,
    }))
}
