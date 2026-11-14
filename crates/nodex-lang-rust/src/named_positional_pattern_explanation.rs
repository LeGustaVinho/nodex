//! Explain positional destructuring with a named tuple-shaped pattern.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" { return None; }
    let path = node.child_by_field_name("type")?;
    let pieces: Vec<_> = named_children(node).into_iter().filter(|part| *part != path && !part.is_extra()).collect();
    // The focused Some/Ok/Err pattern helpers handle one-part patterns separately.
    if pieces.len() < 2 { return None; }
    let site = children(node).into_iter().find(|part| part.kind() == "(")?;
    Some((site, ReadingContent::Detail {
        label: "Match named positional fields".into(),
        text: format!("The pattern {} uses {} to identify a tuple-shaped constructor or variant, then matches the items in parentheses against its fields by position. Without a .. rest pattern, the first matches field 0, the second matches field 1, and so on. A plain name such as r binds the corresponding part on a successful match. If {} resolves to an enum variant, the value must have that variant; if it resolves to a tuple struct, the type must match. The written name alone does not establish which one it is. Bindings may copy, move, or borrow parts depending on their types and pattern context; nested patterns and .. have their own matching rules.", src.norm(node), src.norm(path), src.norm(path)),
        definition: None,
        borrow: false,
    }))
}
