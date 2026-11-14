//! Explain an open-ended range used to index a value, without guessing its type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let parts: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    if parts.len() != 2 { return None; }
    let receiver = parts[0];
    let range = crate::syntax::unparen(parts[1]);
    if range.kind() != "range_expression" || range.has_error() { return None; }
    let operator = children(range).into_iter().find(|child| child.kind() == "..")?;
    let bounds: Vec<_> = named_children(range).into_iter().filter(|part| !part.is_extra()).collect();
    if bounds.len() != 1 || bounds[0].end_byte() > operator.start_byte() { return None; }
    let start = bounds[0];
    let bracket = children(node).into_iter().find(|child| child.kind() == "[" && !child.is_missing())?;
    Some((bracket, ReadingContent::Detail {
        label: "Index from a starting position to the end".into(),
        text: format!("{}[{}..] uses a range starting at {} with no written end as its index. If the receiver is a standard array or slice, positions start at zero and this selects the contiguous suffix from position {} through the end, not a single element. A start equal to the length selects an empty slice; a start past the length panics. The indexed result is a slice place, often borrowed with & or &mut; the indexing syntax by itself does not prove ownership was transferred. Indexing can be overloaded, so behavior for another receiver type depends on its implementation and cannot be established from the name alone.", src.norm(receiver), src.norm(start), src.norm(start), src.norm(start)),
        definition: None,
        borrow: false,
    }))
}
