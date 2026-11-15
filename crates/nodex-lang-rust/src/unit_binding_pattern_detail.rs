//! Explain the empty tuple pattern in a let declaration.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "let_declaration" || node.has_error() || node.child_by_field_name("value").is_none() {
        return None;
    }
    let pattern = node.child_by_field_name("pattern")?;
    if pattern.kind() != "tuple_pattern" || named_children(pattern).iter().any(|child| !matches!(child.kind(), "line_comment" | "block_comment")) {
        return None;
    }
    Some((pattern, format!("The pattern {} matches the unit value `()`, the only value of the unit type `()`. In `let () = ();`, the right side constructs that value; the left side checks its empty tuple shape and introduces no names. More generally, the right-hand expression is evaluated first, including any effects, and must have a type compatible with this pattern. This is an empty tuple pattern, not the wildcard `_`, which can match values of other types without binding them.", src.norm(pattern))))
}
