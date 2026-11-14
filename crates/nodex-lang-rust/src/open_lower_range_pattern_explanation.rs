//! Explain a lower-bounded, open-ended range in a pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() { return None; }
    let lower = node.child_by_field_name("left")?;
    if node.child_by_field_name("right").is_some() { return None; }
    let dots = children(node).into_iter().find(|child| child.kind() == "..")?;
    Some((dots, ReadingContent::Detail {
        label: "Match from a lower bound".into(),
        text: format!("In a pattern, `{}..` matches a value equal to {} or greater. The lower bound is included; the omitted upper bound means there is no additional upper limit from this pattern, though the value's type still limits which values exist. This checks whether the value matches; it does not create or iterate a range. The bound must be valid for the matched type.", src.norm(lower), src.norm(lower)),
        definition: None,
        borrow: false,
    }))
}
