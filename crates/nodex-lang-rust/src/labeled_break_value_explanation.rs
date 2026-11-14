//! Explain `break 'label value` without guessing the target's resolved identity.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "break_expression" || node.has_error() { return None; }
    let parts = named_children(node);
    let label = parts.iter().copied().find(|part| part.kind() == "label")?;
    let value = parts.iter().copied().find(|part| part.kind() != "label" && !part.is_extra())?;
    let name = src.norm(label);
    Some((label, ReadingContent::Detail {
        label: "Leave a named destination with a value".into(),
        text: format!("Evaluate {} and exit the enclosing destination named {name}, skipping the rest of the current and any intervening inner bodies. That value becomes the result of the targeted `loop` expression or labeled block; execution continues after that destination. The apostrophe marks a label, not a string or character. The label must identify an enclosing destination, and a value-bearing break cannot target a `while` or `for` loop. All possible result values must have compatible types.", src.norm(value)),
        definition: None,
        borrow: false,
    }))
}
