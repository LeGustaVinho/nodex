//! Explain empty-braced construction when a path resolves to a unit variant.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "struct_expression" || node.has_error() { return None; }
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && node.end_byte() <= left.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let path = node.child_by_field_name("name")?;
    if !matches!(path.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    path.child_by_field_name("path")?;
    let variant = path.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if !named_children(body).into_iter().filter(|part| !part.is_extra()).collect::<Vec<_>>().is_empty() { return None; }
    Some((variant, ReadingContent::Detail {
        label: "Possible unit enum variant construction".into(),
        text: format!("If `{}` resolves to an enum's unit variant, `{} {{}}` constructs a value of that enum in the `{}` case. A unit variant carries no fields, so the braces contain no values; Rust also permits the plain variant path as a value. Empty braces can instead construct another kind of item, such as an empty struct or a named-field variant with no fields. The written path alone does not establish which declaration it resolves to, or whether construction is permitted here.", src.norm(path), src.norm(path), src.norm(variant)),
        definition: None, borrow: false,
    }))
}
