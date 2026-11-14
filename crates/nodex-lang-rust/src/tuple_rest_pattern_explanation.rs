//! Explain `..` in tuple patterns, without confusing ranges or slice patterns.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "tuple_pattern" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let index = parts.iter().position(|part| part.kind() == "remaining_field_pattern")?;
    let rest = parts[index];
    Some((rest, ReadingContent::Detail {
        label: "Skip middle tuple positions".into(),
        text: format!("In the tuple pattern {}, `..` stands for any number of remaining tuple positions, including zero. The {} written pattern(s) before it match positions from the start; the {} after it match positions from the end. For `(x, .., y)`, x matches the first element and y the last, while the middle elements are not bound by `..`. The tuple must have enough elements for the written positions, and only one `..` is allowed at this level of a tuple pattern. The remaining written patterns and their types determine whether their bindings move, copy, or borrow values. This `..` is a rest pattern, not a range expression.", src.norm(node), index, parts.len() - index - 1),
        definition: None,
        borrow: false,
    }))
}
