//! Explain exact-length array/slice patterns such as `[a, b, c]`.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn top_level_rest(node: Node<'_>) -> bool {
    node.kind() == "remaining_field_pattern" || (node.kind() == "captured_pattern" && named_children(node).iter().any(|child| child.kind() == "remaining_field_pattern"))
}

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "slice_pattern" || node.has_error() { return None; }
    let elements: Vec<_> = named_children(node).into_iter().filter(|child| !child.is_extra()).collect();
    if elements.iter().any(|child| top_level_rest(*child)) { return None; }
    let bracket = children(node).into_iter().find(|child| child.kind() == "[")?;
    let mut anchor = bracket;
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == "match_arm" {
            if let Some(owner) = current.parent().and_then(|body| body.parent()) {
                if let Some(keyword) = children(owner).into_iter().find(|part| part.kind() == "match") { anchor = keyword; }
            }
            break;
        }
        if matches!(current.kind(), "let_condition" | "let_declaration" | "for_expression" | "parameter" | "closure_parameters") { break; }
        parent = current.parent();
    }
    let count = elements.len();
    Some((anchor, bracket, ReadingContent::Detail {
        label: "Match an exact-length sequence".into(),
        text: format!("The pattern {} matches an array or slice with exactly {count} element(s), because there is no `..` rest pattern at this level. The first inner pattern applies to index 0, the next to index 1, and so on; each can bind, ignore, or test its element. For a slice of a different length this pattern does not match. An array's length is part of its type, so an incompatible array length is rejected during compilation. Whether bindings copy, move, or borrow depends on the patterns and types.", src.norm(node)),
        definition: None, borrow: false,
    }))
}
