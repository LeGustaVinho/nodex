//! Explain an `if let` condition without assuming what its pattern names resolve to.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "if_expression" || node.has_error() { return None; }
    let condition = crate::syntax::unparen(node.child_by_field_name("condition")?);
    // Let chains have additional short-circuiting and are described separately.
    if condition.kind() != "let_condition" { return None; }
    let pattern = condition.child_by_field_name("pattern")?;
    let value = condition.child_by_field_name("value")?;
    let keyword = children(node).into_iter().find(|child| child.kind() == "if" && !child.is_missing())?;
    Some((keyword, ReadingContent::Detail {
        label: "Branch by matching a pattern".into(),
        text: format!("Evaluate {} once, then check its value against the pattern {}. If it matches, extract any parts named by the pattern and run the if body; those names are available in that body. If it does not match, skip the if body and run the else branch if one is present; otherwise continue after the if. Matching can move, copy, or borrow parts depending on the pattern and their types. The written pattern alone does not establish which item its names resolve to.", src.norm(value), src.norm(pattern)),
        definition: None,
        borrow: false,
    }))
}
