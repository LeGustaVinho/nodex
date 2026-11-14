//! Explain explicit mutable-reference patterns, distinct from `ref mut` bindings.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "reference_pattern" || node.has_error() { return None; }
    let parts = children(node);
    if !parts.iter().any(|part| part.kind() == "mutable_specifier") { return None; }
    let amp = parts.iter().copied().find(|part| part.kind() == "&")?;
    let inner = named_children(node).into_iter().find(|part| part.kind() != "mutable_specifier")?;
    let mut anchor = amp;
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == "match_arm" {
            if let Some(owner) = current.parent().and_then(|body| body.parent()) {
                if let Some(keyword) = children(owner).into_iter().find(|part| part.kind() == "match") { anchor = keyword; }
            }
            break;
        }
        if matches!(current.kind(), "let_condition" | "let_declaration" | "for_expression" | "parameter" | "closure_parameters") { break; }
        parent = current.parent();
    }
    Some((anchor, amp, ReadingContent::Detail {
        label: "Match through a mutable reference".into(),
        text: format!("The pattern {} matches a mutable reference and applies the inner pattern {} to the referenced value. In `&mut x`, x is a binding by value, not automatically a mutable reference or a reassignable binding: its value is copied when its type is Copy, and moving a non-Copy value out through a borrow is generally not permitted. A nested pattern can bind in other ways. This differs from `ref mut x`, which creates an exclusive mutable reference to the matched part. `mut x` instead makes the local binding reassignable.", src.norm(node), src.norm(inner)),
        definition: None, borrow: false,
    }))
}
