//! Explain named bounds of an inclusive range pattern in a match arm.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "range_pattern" || node.has_error() { return None; }
    let low = node.child_by_field_name("left")?;
    let high = node.child_by_field_name("right")?;
    if !matches!(low.kind(), "identifier" | "scoped_identifier") || !matches!(high.kind(), "identifier" | "scoped_identifier") { return None; }
    if !children(node).iter().any(|n| n.kind() == "..=") { return None; }
    let mut current = node;
    let arm = loop {
        current = current.parent()?;
        if current.kind() == "match_arm" { break current; }
        if matches!(current.kind(), "block" | "match_expression") { return None; }
    };
    let pattern = arm.child_by_field_name("pattern")?;
    if node.start_byte() < pattern.start_byte() || node.end_byte() > pattern.end_byte() { return None; }
    let owner = arm.parent()?.parent().filter(|p| p.kind() == "match_expression")?;
    let anchor = children(owner).into_iter().find(|part| part.kind() == "match")?;
    Some((anchor, low, ReadingContent::Detail {
        label: "Match between named bounds".into(),
        text: format!("`{}..={}` is a range pattern with named lower and upper bounds, both included. It checks whether the matched value falls between those bounds; it does not iterate. Rust must resolve the endpoint paths to allowed constant values and check their types and ordering. The spelling alone does not establish what {} and {} resolve to, their values, or whether the pattern is valid. Match arms are tried in order; if this pattern does not match, a following `_` arm can handle the remaining values.", src.norm(low), src.norm(high), src.norm(low), src.norm(high)),
        definition: None, borrow: false,
    }))
}
