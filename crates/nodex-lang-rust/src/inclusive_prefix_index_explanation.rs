//! Explain indexing by a range with an omitted start and inclusive end.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "index_expression" || node.has_error() { return None; }
    let parts = named_children(node);
    if parts.len() != 2 { return None; }
    let target = parts[0];
    let index = crate::syntax::unparen(parts[1]);
    let end = if let Some((_, end)) = crate::syntax::inclusive_range_to(index) {
        end
    } else if index.kind() == "range_expression" {
        let tokens = children(index);
        let dots = tokens.iter().find(|part| part.kind() == "..=")?;
        if tokens.iter().any(|part| part.is_named() && part.end_byte() <= dots.start_byte()) { return None; }
        tokens.iter().copied().find(|part| part.is_named() && part.start_byte() >= dots.end_byte())?
    } else { return None; };
    let bracket = children(node).into_iter().find(|part| part.kind() == "[")?;
    Some((bracket, ReadingContent::Detail {
        label: "Index through an inclusive end".into(),
        text: format!("`{}[..={}]` applies the written range as an index to {}. For a standard array or slice, the missing start means index 0 and the end {} is included: it selects elements from the beginning through that index. These are zero-based indexes; for example, `[..=4]` selects the first five elements. A standard array or slice operation panics if the end index is outside its length. Standard string slicing additionally requires valid UTF-8 character boundaries. Other types may implement indexing differently, so the syntax alone does not prove the receiver's type, result, or behavior.", src.norm(target), src.norm(end), src.norm(target), src.norm(end)),
        definition: None,
        borrow: false,
    }))
}
