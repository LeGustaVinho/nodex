//! Explain the syntax of a synchronous closure with an explicit return type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" || children(node).iter().any(|part| part.kind() == "async") { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let result = node.child_by_field_name("return_type")?;
    let arrow = children(node).into_iter().find(|part| part.kind() == "->")?;
    let body = node.child_by_field_name("body")?;
    if body.kind() != "block" { return None; }
    Some((arrow, ReadingContent::Detail {
        label: "Closure with an explicit result type".into(),
        text: format!("{} lists the closure's inputs between vertical bars; a colon after a parameter name introduces its type. The arrow -> declares that calling this closure must produce {}. The braces contain the body, which runs when the closure is called. A final expression without a semicolon supplies the block's value; an explicit return can also return a value. Rust checks that returned values match the declared type. Creating the closure and calling it are separate steps.", src.norm(parameters), src.norm(result)),
        definition: None,
        borrow: false,
    }))
}
