//! Explain the block required after an explicit closure result type.
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "closure_expression" || node.has_error() { return None; }
    node.child_by_field_name("return_type")?;
    let body = node.child_by_field_name("body")?;
    if body.kind() != "block" { return None; }
    let arrow = children(node).into_iter().find(|child| child.kind() == "->")?;
    Some((arrow, "A closure that writes an explicit result type after `->` must use a brace-delimited block for its body: `|x: i32| -> i32 { x + 1 }`. The braces are required even when the body contains only one expression. Inside the block, a final expression without a semicolon supplies its value; adding a semicolon changes that block value to `()`, unless control exits earlier. When the result type is omitted, a closure may instead use an expression directly, as in `|x: i32| x + 1`. The compiler checks that the body is compatible with any declared result type.".into()))
}
