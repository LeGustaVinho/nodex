//! Explain the written `Ok(value)` pattern without guessing name resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" || node.has_error() { return None; }
    let path = node.child_by_field_name("type")?;
    let name = if path.kind() == "identifier" { path } else { path.child_by_field_name("name")? };
    if src.norm(name) != "Ok" { return None; }
    let payload: Vec<_> = named_children(node).into_iter().filter(|part| *part != path && !part.is_extra()).collect();
    if payload.len() != 1 { return None; }
    let open = children(node).into_iter().find(|part| part.kind() == "(")?;
    Some((open, ReadingContent::Detail {
        label: "Match an Ok-shaped value".into(),
        text: format!("The pattern {} checks for the written Ok variant or tuple-like constructor and, if it matches, applies the inner pattern {} to its one contained part. A simple name there introduces a binding for that part; `_` ignores it and a nested pattern can check it further. If Ok resolves to the standard Result variant, Ok holds a successful result and the alternative Err holds an error. Rust must resolve the name and check the type, so the spelling alone does not prove it is the standard Result. The inner pattern and types determine whether the part is moved, copied, or borrowed.", src.norm(node), src.norm(payload[0])),
        definition: None,
        borrow: false,
    }))
}
