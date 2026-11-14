//! Beginner-facing explanation of `if` conditions combining `let` patterns.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children};
use tree_sitter::Node;

pub(crate) fn explain(node: Node<'_>) -> Option<(Node<'_>, ReadingContent)> {
    if node.kind() != "let_chain" || node.has_error() { return None; }
    let mut enclosing = node.parent()?;
    while enclosing.kind() == "parenthesized_expression" { enclosing = enclosing.parent()?; }
    if enclosing.kind() != "if_expression" || enclosing.child_by_field_name("condition")? != node { return None; }
    let checks = named_children(node).into_iter().filter(|part| !part.is_extra()).collect::<Vec<_>>();
    if checks.iter().filter(|part| part.kind() == "let_condition").count() < 2 { return None; }
    let operator = children(node).into_iter().find(|child| child.kind() == "&&")?;
    Some((operator, ReadingContent::Detail {
        label: "Match conditions in sequence".into(),
        text: "Check the conditions from left to right. Each `let` evaluates its expression and checks its pattern; a successful match creates the names in that pattern, available to later conditions and the if body. `&&` stops the checks at the first failure, so later expressions do not run. The if body runs only if every check succeeds. On failure, skip that body and run the else branch if one exists; names introduced by the condition are not available in else or after the if.".into(),
        definition: None,
        borrow: false,
    }))
}
