//! Explain the `/=` compound assignment without assuming operand types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn reading<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" || node.has_error() { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "/=" { return None; }
    let target = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    Some((operator, ReadingContent::Detail {
        label: "Divide and assign to an existing place".into(),
        text: format!("{} /= {} updates the existing writable place {} using the right-hand value. Rust applies the applicable DivAssign operation for the operand types; a user-defined implementation can define its own behavior, so this syntax alone does not establish the numeric types or result. For primitive integers, division truncates toward zero and division by zero panics; floating-point behavior differs. Compound assignment does not declare a new variable. Do not assume it is equivalent in every case to writing the place twice as `place = place / value`: overloads and operand evaluation can differ. The assignment expression itself produces () (unit).", src.norm(target), src.norm(right), src.norm(target)),
        definition: None,
        borrow: false,
    }))
}
