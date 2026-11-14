//! Explain the syntax of a single `match` arm.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the containing match and the arm's arrow so the detail can attach to
/// the match node while highlighting the relevant arm.
pub(crate) fn explain<'t>(src: Src<'_>, arm: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if arm.kind() != "match_arm" || arm.has_error() { return None; }
    let body = arm.parent()?;
    let match_node = body.parent().filter(|node| node.kind() == "match_expression")?;
    let pattern = arm.child_by_field_name("pattern")?;
    let value = arm.child_by_field_name("value")?;
    let arrow = children(arm).into_iter().find(|part| part.kind() == "=>" && !part.is_missing())?;
    Some((match_node, arrow, ReadingContent::Detail {
        label: "A match arm selects an expression".into(),
        text: format!("The pattern {} describes when this arm is eligible. Rust tries arms in order: when the pattern matches and any `if` guard succeeds, it selects this arm and evaluates {} after `=>`; later arms are skipped. If that expression completes, its value becomes the value of the whole match. A comma separates arms; after a block expression it can be omitted. Patterns may introduce names used in the selected arm.", src.norm(pattern), src.norm(value)),
        definition: None,
        borrow: false,
    }))
}
