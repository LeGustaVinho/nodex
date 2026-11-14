//! Explain an identifier bound by `@` to an inclusive range pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "captured_pattern" { return None; }
    let parts = named_children(node).into_iter().filter(|part| !part.is_extra()).collect::<Vec<_>>();
    let [binding, range] = parts.as_slice() else { return None };
    if binding.kind() != "identifier" || range.kind() != "range_pattern" { return None; }
    if !children(*range).iter().any(|part| part.kind() == "..=") { return None; }
    let site = children(node).into_iter().find(|part| part.kind() == "@")?;
    let bounds = named_children(*range);
    let (Some(start), Some(end)) = (bounds.first(), bounds.last()) else { return None; };
    if bounds.len() != 2 { return None; }
    Some((site, ReadingContent::Detail {
        label: "Bind a value in an inclusive range".into(),
        text: format!("The pattern {} first checks whether the value is between {} and {}, including both endpoints. If it matches, @ binds the matched value to {} so that the name can be used in the successful arm or guard. If the value is outside that range, this pattern does not match and the binding is not available on that path. The binding's move, copy, or borrow behavior depends on the matched value and pattern context.", src.norm(node), src.norm(*start), src.norm(*end), src.norm(*binding)),
        definition: None,
        borrow: false,
    }))
}
