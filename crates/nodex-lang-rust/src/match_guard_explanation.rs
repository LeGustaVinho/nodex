//! Explain the boolean guard on a `match` arm.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Return the owning match and its `if` token for a guarded arm.
pub(crate) fn explain<'t>(src: Src<'_>, arm: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if arm.kind() != "match_arm" || arm.has_error() { return None; }
    let pattern = arm.child_by_field_name("pattern")?;
    let condition = pattern.child_by_field_name("condition")?;
    let keyword = children(pattern).into_iter().find(|child| child.kind() == "if")?;
    let owner = arm.parent()?.parent().filter(|parent| parent.kind() == "match_expression")?;
    Some((owner, keyword, ReadingContent::Detail {
        label: "Check a match guard".into(),
        text: format!("First try the pattern on the value being matched. Only if it matches, evaluate the guard {} after `if`; names bound by the pattern are available in that condition. If the guard is true, run the expression after `=>` for this arm. If it is false, continue looking for another matching alternative or a later arm; the expression for this attempted match does not run. Evaluating the guard can have effects, and an early exit or panic in it follows its normal control flow.", src.norm(condition)),
        definition: None,
        borrow: false,
    }))
}
