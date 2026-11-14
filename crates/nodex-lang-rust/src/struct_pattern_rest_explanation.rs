//! Explain ignored remaining fields in a struct-like pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_pattern" || node.has_error() { return None; }
    let rest = named_children(node).into_iter().find(|child| child.kind() == "remaining_field_pattern")?;
    Some((rest, ReadingContent::Detail {
        label: "Ignore remaining fields".into(),
        text: format!("In the pattern `{}`, the fields written explicitly are matched and can bind names; for example, shorthand `x` means `x: x`, binding the x field to a local name x. The `..` allows the other fields to be omitted from this pattern: they are not bound by it. The pattern describes a struct or struct-style enum variant; it does not construct a value. How a written binding copies, moves, or borrows a field depends on its type and binding mode.", src.norm(node)),
        definition: None,
        borrow: false,
    }))
}
