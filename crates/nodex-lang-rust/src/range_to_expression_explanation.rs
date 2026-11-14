//! Explain an exclusive upper-bounded range without assuming how it is used.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::{syntax::{children, Src}, syntax_sites::rust_assignee_rest};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "range_expression" || node.has_error() || rust_assignee_rest(node) { return None; }
    let parts = children(node);
    let operator = parts.iter().copied().find(|part| part.kind() == "..")?;
    if parts.iter().any(|part| part.is_named() && !part.is_extra() && part.end_byte() <= operator.start_byte()) { return None; }
    let end = parts.iter().copied().find(|part| part.is_named() && !part.is_extra() && part.start_byte() >= operator.end_byte())?;
    Some((end, ReadingContent::Detail {
        label: "Range with an exclusive upper bound".into(),
        text: format!("`..{}` creates a range value with no lower bound written and an upper bound that is excluded. The end expression is evaluated when the range value is constructed. A value equal to the upper bound is outside this range. The omitted start does not generally mean zero; what the range does depends on how it is used. This range is not itself a sequence that can be directly iterated with `for`.", src.norm(end)),
        definition: None,
        borrow: false,
    }))
}
