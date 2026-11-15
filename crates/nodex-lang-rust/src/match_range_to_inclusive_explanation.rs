//! Explain a match pattern with an omitted lower bound and inclusive upper bound.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() || node.child_by_field_name("left").is_some() { return None; }
    let upper = node.child_by_field_name("right")?;
    let operator = children(node).into_iter().find(|child| child.kind() == "..=")?;
    let mut current = node.parent();
    let mut in_arm = false;
    let owner = loop {
        let ancestor = current?;
        if ancestor.kind() == "match_arm" { in_arm = true; }
        if ancestor.kind() == "match_expression" { break in_arm.then_some(ancestor)?; }
        if matches!(ancestor.kind(), "function_item" | "closure_expression" | "async_block") { return None; }
        current = ancestor.parent();
    };
    let end = src.norm(upper);
    Some((owner, operator, ReadingContent::Detail {
        label: "Pattern at or below an inclusive limit".into(),
        text: format!("In a match pattern, `..={end}` matches a value less than or equal to {end}; the `=` means {end} itself is included. For `..=0`, zero and representable negative values match, while positive values do not. No lower endpoint is written, but the matched type still limits its possible values. The endpoint must be valid as a constant pattern bound for that type. This tests the value when selecting an arm; it does not construct or iterate a range. Earlier arms and any guard can affect which arm is selected."),
        definition: None, borrow: false,
    }))
}
