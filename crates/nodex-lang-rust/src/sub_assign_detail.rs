//! Explain the subtraction compound-assignment operator without assuming types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "compound_assignment_expr" { return None; }
    let operator = node.child_by_field_name("operator")?;
    if src.text(operator) != "-=" { return None; }
    let place = src.norm(node.child_by_field_name("left")?);
    let value = src.norm(node.child_by_field_name("right")?);
    Some((operator, ReadingContent::Detail {
        label: "Subtract and assign".into(),
        text: format!("`{place} -= {value}` applies subtraction assignment to the mutable place {place} using {value}; for ordinary numeric values it subtracts the right-hand value from the current left-hand value and updates that place. Rust also supports custom behavior through the applicable `SubAssign` trait implementation, so the exact operation depends on the operand types and resolution. The left side must be a mutable place, and the assignment expression itself produces `()`. Operand evaluation order depends on the operand types: when both are known to be primitive before monomorphization, Rust evaluates the right side first; other cases follow different rules. The written operator alone does not establish the types, selected implementation, overflow behavior, or evaluation order."),
        definition: None, borrow: false,
    }))
}
