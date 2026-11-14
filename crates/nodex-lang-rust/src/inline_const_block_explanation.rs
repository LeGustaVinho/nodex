//! Explain an inline const block where the expression occurs.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "const_block" || node.has_error() { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "const")?;
    Some((keyword, ReadingContent::Detail {
        label: "Evaluate an inline constant".into(),
        text: "`const { ... }` is an expression whose block is evaluated in a compile-time constant context, rather than running its computations as ordinary runtime code. The final expression without a semicolon supplies the block's value; without one the value is `()` (unit). The compiler restricts what operations are allowed in this context. An inline const can refer to generic parameters in scope, but cannot read ordinary local runtime variables. If execution reaches the expression, its value must have been computed successfully at compile time; an unused or unreachable block need not necessarily be evaluated. This form does not declare a named constant.".into(),
        definition: None,
        borrow: false,
    }))
}
