//! Explain a range expression with a start and no written ending bound.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use nodex_lang_api::syntax_sites::rust_assignee_rest;
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "range_expression" || node.has_error() || rust_assignee_rest(node) { return None; }
    let tokens = children(node);
    let op = tokens.iter().copied().find(|child| child.kind() == "..")?;
    let bounds: Vec<_> = tokens.iter().copied().filter(|child| child.is_named() && !child.is_extra()).collect();
    if bounds.len() != 1 || bounds[0].end_byte() > op.start_byte() { return None; }
    let start = bounds[0];
    if matches!(start.kind(), "return_expression" | "break_expression" | "continue_expression" | "yield_expression") { return None; }
    Some((start, ReadingContent::Detail {
        label: "Range from a starting value".into(),
        text: format!("{} is the included starting bound of this range; `..` has no written upper bound. Creating the range describes bounds and does not itself iterate. For integer iteration it starts at this value, then advances; the finite integer type and overflow behavior mean it should not be assumed to run safely forever. When used for built-in array, slice, or string indexing, the omitted end means the collection's end, subject to bounds checks and, for strings, UTF-8 character-boundary rules.", src.norm(start)),
        definition: None, borrow: false,
    }))
}
