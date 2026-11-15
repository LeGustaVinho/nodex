//! Explain method-call receiver lookup without guessing the selected method.
//! Rust Reference: https://doc.rust-lang.org/reference/expressions/method-call-expr.html
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "call_expression" || node.has_error() { return None; }
    let (receiver, method) = crate::expr::method_call(node)?;
    if method.kind() != "field_identifier" { return None; }
    let mut function = node.child_by_field_name("function")?;
    if function.kind() == "generic_function" { function = function.child_by_field_name("function")?; }
    let dot = children(function).into_iter().find(|part| part.kind() == ".")?;
    let arguments = node.child_by_field_name("arguments")?;
    Some((dot, ReadingContent::Detail {
        label: "Look up and call a method".into(),
        text: format!("In this call, `{}` is the receiver, `{}` names the method, and `{}` contains the explicit arguments. The selected method also receives the receiver as its `self` argument. Rust resolves the method during compilation, considering methods available on receiver types and traits in scope. As part of lookup it can automatically dereference the receiver and may insert a shared or mutable borrow (`&` or `&mut`) when appropriate; the required access must be valid. A by-value receiver can be moved or copied according to its type. The spelling alone does not prove which method, adjustments, or dispatch strategy is selected.", src.norm(receiver), src.norm(method), src.norm(arguments)),
        definition: None,
        borrow: false,
    }))
}
