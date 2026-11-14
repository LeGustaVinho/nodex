//! Syntax-only explanation of Rust's question-mark propagation operator.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "try_expression" || node.has_error() { return None; }
    let operand = named_children(node).into_iter().next()?;
    let token = children(node).into_iter().find(|child| child.kind() == "?" && !child.is_missing())?;
    Some((operand, token, ReadingContent::Detail {
        label: "Continue or propagate with ?".into(),
        text: format!("Evaluate {} once. The ? operator checks the value using its type's propagation behavior. On the continue path it extracts the output, which becomes the value of this expression. On the other path it stops the current path and propagates the residual out of the enclosing compatible function, closure, async block, or try block; conversion may be required by that context. For the standard Result type these paths correspond to Ok and Err; for Option they correspond to Some and None. The syntax alone does not establish which type or propagation behavior applies here.", src.norm(operand)),
        definition: None,
        borrow: false,
    }))
}
