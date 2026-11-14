//! Explain a slice pattern that binds its middle section in a match arm.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "slice_pattern" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let [first, middle, last] = parts.as_slice() else { return None; };
    if middle.kind() != "captured_pattern" { return None; }
    let captured = named_children(*middle);
    let [name, rest] = captured.as_slice() else { return None; };
    if name.kind() != "identifier" || rest.kind() != "remaining_field_pattern" { return None; }
    let at = children(*middle).into_iter().find(|part| part.kind() == "@")?;
    let mut current = node;
    let arm = loop {
        current = current.parent()?;
        if current.kind() == "match_arm" { break current; }
        if matches!(current.kind(), "block" | "match_expression") { return None; }
    };
    let arm_pattern = arm.child_by_field_name("pattern")?;
    if node.start_byte() < arm_pattern.start_byte() || node.end_byte() > arm_pattern.end_byte() { return None; }
    let owner = arm.parent()?.parent().filter(|parent| parent.kind() == "match_expression")?;
    Some((owner, at, ReadingContent::Detail {
        label: "Bind the middle of a sequence".into(),
        text: format!("The pattern {} matches a sequence with at least two elements. It matches {} against the first element and {} against the last. The `..` matches all elements between those ends, including an empty middle; `{} @ ..` binds that middle section to {}. The bindings are available on the successful arm path. Whether a binding moves, copies, or borrows data depends on the matched value, its type, and the binding patterns.", src.norm(node), src.norm(*first), src.norm(*last), src.norm(*name), src.norm(*name)),
        definition: None,
        borrow: false,
    }))
}
