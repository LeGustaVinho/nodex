//! Explain a block whose final expression is terminated with a semicolon.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// The semicolon turns the last expression into a statement, so its result is
/// discarded. Diverging expressions remain divergent; unit is conditional on
/// actually reaching the end of the block.
pub(crate) fn explain<'t>(src: Src<'_>, block: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if block.kind() != "block" || block.has_error() { return None; }
    let statement = crate::syntax::block_statements(block).next_back()?;
    if statement.kind() != "expression_statement" { return None; }
    let semicolon = children(statement).into_iter().find(|child| child.kind() == ";" && !child.is_missing())?;
    let expression = statement.named_child(0)?;
    if expression.is_extra() { return None; }
    Some((statement, semicolon, ReadingContent::Detail {
        label: "A semicolon makes the block yield unit".into(),
        text: format!("Evaluate {} for its effects, then discard its result because the expression ends with `;`. If execution reaches the end of this block normally, the block's value is `()` (unit), the one value of the unit type. Without this final semicolon, the expression's value would be the block's value. If the expression returns, breaks, panics, or otherwise does not reach the end, the block does not produce unit on that path.", src.norm(expression)),
        definition: None,
        borrow: false,
    }))
}
