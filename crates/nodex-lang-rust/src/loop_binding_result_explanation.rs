//! Explain a `loop` expression used as a local binding's initializer.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "loop_expression" || node.has_error() { return None; }
    let mut value = node;
    while let Some(parent) = value.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
        value = parent;
    }
    let declaration = value.parent().filter(|parent| parent.kind() == "let_declaration")?;
    if declaration.child_by_field_name("value") != Some(value) { return None; }
    let pattern = declaration.child_by_field_name("pattern")?;
    if pattern.kind() != "identifier" { return None; }
    let keyword = children(declaration).into_iter().find(|child| child.kind() == "let" && !child.is_missing())?;
    let name = src.norm(pattern);
    Some((keyword, ReadingContent::Detail {
        label: "Bind the result of a loop".into(),
        text: format!("A loop can produce a value. Start the loop body and repeat it until control leaves. A break carrying a value and targeting this loop ends it and supplies that value as the result of the loop expression; let then binds that result to {name}. For example, break 42 makes the loop produce 42. If this loop never completes, execution does not reach the binding after the initializer. A break aimed at a different labeled loop or block does not supply this loop's result."),
        definition: None,
        borrow: false,
    }))
}
