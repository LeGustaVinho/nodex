//! Explain a `Some(x)` tuple-style pattern without assuming name resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_struct_pattern" { return None; }
    let path = node.child_by_field_name("type")?;
    let name = src.norm(path);
    if name != "Some" && !name.ends_with("::Some") { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| *part != path && !part.is_extra()).collect();
    if parts.len() != 1 { return None; }
    let inner = src.norm(parts[0]);
    Some((path, ReadingContent::Detail {
        label: "Match a Some-shaped value".into(),
        text: format!("The pattern {} checks that the input has the tuple-style form named {name}; only if it matches does the inner pattern {inner} match its contained part. If {name} resolves to the standard Option::Some variant, this means the option contains a value; a different variant such as None does not match. The spelling alone does not prove which item {name} names. An identifier inside the pattern creates a binding; whether it moves, copies, or borrows the part depends on the pattern, the input and its type. The binding is available only along the successful match path.", src.norm(node)),
        definition: None,
        borrow: false,
    }))
}
