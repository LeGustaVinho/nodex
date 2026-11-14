//! Beginner-facing overview of a `while let` loop.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "while_expression" || node.has_error() { return None; }
    let condition = crate::syntax::unparen(node.child_by_field_name("condition")?);
    // A let-chain has additional short-circuit rules and is explained separately.
    if condition.kind() != "let_condition" { return None; }
    let pattern = condition.child_by_field_name("pattern")?;
    let value = condition.child_by_field_name("value")?;
    let keyword = children(condition).into_iter().find(|child| child.kind() == "let" && !child.is_missing())?;
    Some((keyword, ReadingContent::Detail {
        label: "Repeat while a pattern matches".into(),
        text: format!("Before each iteration, evaluate {} again and compare its result with the pattern {}. If it matches, bind the names extracted by the pattern and run the body; those bindings are available inside that iteration. When the body finishes normally, start the check again. If the pattern does not match, skip the body and continue after the loop. A break or return can also leave earlier. For a standard iterator, next() normally yields Some(item) for an item and None at the end, so `while let Some(x) = iter.next()` processes items until the first None. The names and method must resolve to those standard meanings for that example to apply.", src.norm(value), src.norm(pattern)),
        definition: None,
        borrow: false,
    }))
}
