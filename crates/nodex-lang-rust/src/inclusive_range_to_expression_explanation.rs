//! Explain range-to-inclusive expressions without confusing them with patterns.
use nodex_ir::{reading::ReadingContent, Span};
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    // Tree-sitter may parse `..=end` as an assignment expression; the shared
    // recognizer requires adjacent dots and equals, so ordinary assignment is excluded.
    let (equal, end) = crate::syntax::inclusive_range_to(node)?;
    Some((src.span(equal), ReadingContent::Detail {
        label: "Create a range through an inclusive end".into(),
        text: format!("`..={}` creates a range value with no lower bound written and an upper bound that includes {} itself. Omitting the start does not mean the range starts at zero in every context. For slicing, it selects from the beginning through that index, provided the index is valid; string slices also require UTF-8 character boundaries. Evaluating this expression evaluates its endpoint and creates a range value; it does not enumerate values or create a list. This is an expression, whereas a range pattern tests a value for a match.", src.norm(end), src.norm(end)),
        definition: None,
        borrow: false,
    }))
}
