//! Explain `if ... else` as a value-producing choice.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// A normal Boolean `if` with an explicit alternative. Pattern-based `if let`
/// and malformed trees require different descriptions and are intentionally excluded.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "if_expression" || node.has_error() { return None; }
    let condition = node.child_by_field_name("condition")?;
    if matches!(condition.kind(), "let_condition" | "let_chain") { return None; }
    node.child_by_field_name("consequence")?;
    node.child_by_field_name("alternative")?;
    let keyword = children(node).into_iter().find(|child| child.kind() == "if" && !child.is_missing())?;
    Some((keyword, ReadingContent::Detail {
        label: "Choose a value with if and else".into(),
        text: format!("Evaluate {} as a Boolean condition. If it is true, run the first branch; if false, run the else branch. Only the selected branch runs. When that branch completes normally, its final expression without a semicolon supplies the value of the entire if expression; a block without such an expression supplies () (unit). Rust checks that branches which produce values have compatible result types, allowing supported coercions; a branch that exits or never completes does not supply a value. The resulting value can be used in an assignment, returned, or ignored.", src.norm(condition)),
        definition: None,
        borrow: false,
    }))
}
