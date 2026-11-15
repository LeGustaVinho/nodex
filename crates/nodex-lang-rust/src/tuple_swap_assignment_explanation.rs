//! Explain a two-variable tuple assignment that swaps the values.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "assignment_expression" { return None; }
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;
    if left.kind() != "tuple_expression" || right.kind() != "tuple_expression" { return None; }
    let destinations = named_children(left).into_iter().filter(|part| !part.is_extra()).collect::<Vec<_>>();
    let values = named_children(right).into_iter().filter(|part| !part.is_extra()).collect::<Vec<_>>();
    if destinations.len() != 2 || values.len() != 2 || destinations.iter().chain(values.iter()).any(|part| part.kind() != "identifier") { return None; }
    let first = src.norm(destinations[0]);
    let second = src.norm(destinations[1]);
    if first == second || first != src.norm(values[1]) || second != src.norm(values[0]) { return None; }
    Some((node, ReadingContent::Detail {
        label: "Swap two values".into(),
        text: format!("This destructuring assignment swaps the current values of {first} and {second}: afterward {first} holds the old value of {second}, and {second} holds the old value of {first}. Rust evaluates the entire right-hand tuple first, taking its elements in order, before assigning its components to the existing destinations from left to right. Consequently the second assignment uses the saved old value, not the value just written into {first}. The destinations must be assignable and their types compatible. Depending on the types, the values are copied or moved; this syntax alone does not resolve which."),
        definition: None, borrow: false,
    }))
}
