//! Describe the control and completeness rules of a Rust match expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

pub(crate) fn explain<'t>(node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" || node.has_error() { return None; }
    let body = node.child_by_field_name("body")?;
    let keyword = children(node).into_iter().find(|child| child.kind() == "match")?;
    let brace = children(body).into_iter().find(|child| child.kind() == "{")?;
    Some((keyword, brace, ReadingContent::Detail {
        label: "Choose a matching arm".into(),
        text: "Evaluate the expression after match, then check the arms in the order written. The first pattern that matches and whose optional guard is true selects its arm; its expression runs and supplies the value of the match. If a guard is false, matching continues with later possibilities. Rust checks that the patterns cover every possible value (exhaustiveness); `_` can cover remaining cases. A guard can fail, so a guarded arm generally does not by itself establish coverage. A selected arm can also leave the surrounding function or loop instead of yielding a value.".into(),
        definition: None, borrow: false,
    }))
}
