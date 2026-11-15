//! Explain an empty match without assuming the scrutinee's type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" || node.has_error() { return None; }
    let value = node.child_by_field_name("value")?;
    let body = node.child_by_field_name("body")?;
    if named_children(body).into_iter().any(|part| part.kind() == "match_arm") { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "match")?;
    let close = children(body).into_iter().find(|part| part.kind() == "}")?;
    Some((keyword, close, ReadingContent::Detail {
        label: "Match with no arms".into(),
        text: format!("Evaluate {} first; this match has no arms and therefore no branch that could run or produce a value. Rust checks exhaustiveness: an empty match can be accepted when the scrutinee's type is known to have no possible values, subject to the compiler's visibility and exhaustiveness rules. If evaluation never returns, control never reaches an arm either. The syntax alone does not establish the scrutinee's type, whether it is uninhabited, or whether this code compiles.", src.norm(value)),
        definition: None, borrow: false,
    }))
}
