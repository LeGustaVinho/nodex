//! Explain XOR assignment without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "^=" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "XOR and assign".into(),
        text: format!("`{} ^= {}` updates the mutable place on the left using the right-hand value; the assignment expression produces the unit value `()`. For built-in integer operands it performs bitwise exclusive OR: each result bit is 1 when the corresponding input bits differ, and the result is stored in the left place. For booleans it produces true when the operands differ. User-defined types may implement `std::ops::BitXorAssign`, so the operation and right-hand type depend on type checking and the selected implementation. The syntax alone does not establish the operand types or implementation.", src.norm(left), src.norm(right)),
        definition: None, borrow: false,
    }))
}
