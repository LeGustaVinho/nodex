//! Explain a `break 'label` that names its enclosing destination.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "break_expression" { return None; }
    let parts = named_children(node);
    let label = parts.iter().copied().find(|part| part.kind() == "label")?;
    // A value-bearing break has additional behavior handled separately.
    if parts.iter().any(|part| part.kind() != "label" && !part.is_extra()) { return None; }
    let name = src.norm(label);
    Some((label, ReadingContent::Detail {
        label: "Exit a named enclosing destination".into(),
        text: format!("break {name} leaves the enclosing destination marked {name}, skipping any remaining statements on the way out. If that destination is a loop, execution continues after that loop and any inner loops are exited too. Rust also permits labeled blocks as break destinations. The apostrophe is part of the label syntax; it is not a string or character literal. The label must name an enclosing destination in scope."),
        definition: None,
        borrow: false,
    }))
}
