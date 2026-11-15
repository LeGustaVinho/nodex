//! Explain remainder compound assignment without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "%=" { return None; }
    let target = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Assign a remainder to an existing place".into(),
        text: format!("{} %= {} updates the existing writable place {} using the right-hand value. Rust applies the applicable RemAssign operation for the operand types; a user-defined implementation can define its own behavior, so the syntax alone does not establish the numeric types or result. For primitive integers, the remainder has the sign of the dividend or is zero, and a zero divisor panics; floating-point behavior differs. This does not declare a variable. Do not assume it is equivalent in every case to `place = place % value`: overloads and operand evaluation can differ. The assignment expression produces () (unit).", src.norm(target), src.norm(right), src.norm(target)),
        definition: None, borrow: false,
    }))
}
