//! Explain endpoint slice patterns with an ignored middle `..`.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "slice_pattern" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    let positions: Vec<_> = parts.iter().enumerate().filter(|(_, part)| part.kind() == "remaining_field_pattern").map(|(i, _)| i).collect();
    let [index] = positions.as_slice() else { return None; };
    if *index == 0 || *index + 1 >= parts.len() { return None; }
    let rest = parts[*index];
    // This explanation applies to `..`, not `name @ ..` which creates a binding.
    if src.norm(rest) != ".." { return None; }
    let count = parts.len() - 1;
    let mut owner = node.parent()?;
    loop {
        if matches!(owner.kind(), "match_expression" | "if_expression" | "while_expression" | "for_expression" | "let_declaration") { break; }
        if matches!(owner.kind(), "function_item" | "closure_expression" | "source_file") { return None; }
        owner = owner.parent()?;
    }
    Some((owner, rest, ReadingContent::Detail {
        label: "Match sequence endpoints".into(),
        text: format!("In {}, the patterns before `..` match elements from the start and those after `..` match elements from the end. The sequence must have at least {} elements for the explicit positions; the two endpoints therefore cannot be the same element. `..` accepts any number of intervening elements, including none, without binding them. If the sequence is too short, the pattern does not match. How the named parts are copied, moved, or borrowed depends on their binding modes and the matched value's type; the pattern alone does not establish ownership.", src.norm(node), count),
        definition: None,
        borrow: false,
    }))
}
