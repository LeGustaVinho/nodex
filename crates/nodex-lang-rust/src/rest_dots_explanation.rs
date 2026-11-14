//! Explain `..` as a rest pattern, distinct from an unbounded range expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "remaining_field_pattern" || node.has_error() || src.norm(node) != ".." { return None; }
    let pattern = node.parent()?;
    let text = match pattern.kind() {
        "struct_pattern" => format!("In {}, `..` is a rest pattern: it allows the remaining fields to be omitted rather than binding each one. Written fields still have to match, and the omitted fields are not bound by `..`. Visibility and other rules for the matched type still apply. Here the dots are not an unbounded range expression.", src.norm(pattern)),
        "slice_pattern" => {
            let parts: Vec<_> = named_children(pattern).into_iter().filter(|part| !part.is_extra()).collect();
            let pos = parts.iter().position(|part| *part == node)?;
            // Endpoint patterns have a more detailed existing explanation.
            if pos > 0 && pos + 1 < parts.len() { return None; }
            let count = parts.len().saturating_sub(1);
            format!("In {}, `..` is a rest pattern: it matches the remaining sequence elements, including an empty remainder, without binding them. The sequence must contain at least {} {} for the other written patterns. Here the dots are not an unbounded range expression.", src.norm(pattern), count, if count == 1 { "element" } else { "elements" })
        }
        // Tuple rest already has a dedicated detail.
        _ => return None,
    };
    let mut owner = pattern.parent()?;
    loop {
        if matches!(owner.kind(), "match_expression" | "if_expression" | "while_expression" | "for_expression" | "let_declaration") { break; }
        if matches!(owner.kind(), "function_item" | "closure_expression" | "source_file") { return None; }
        owner = owner.parent()?;
    }
    Some((owner, node, ReadingContent::Detail { label: "Ignore remaining pattern parts".into(), text, definition: None, borrow: false }))
}
