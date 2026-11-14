//! Explain postfix `.await` without guessing the operand's concrete future type.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "await_expression" || node.has_error() { return None; }
    let parts = children(node);
    let dot = parts.iter().find(|part| part.kind() == ".").copied()?;
    parts.iter().find(|part| part.kind() == "await")?;
    let operand = named_children(node).into_iter().next()?;
    Some((dot, ReadingContent::Detail {
        label: "Await an asynchronous result".into(),
        text: format!("Evaluate {} first; Rust requires that operand to implement IntoFuture and converts it into a future. The future is polled for progress. If it is pending, the surrounding async computation can suspend until it is woken and polled again; when it is ready, its output becomes the value of this `.await` expression. The `.await` comes after its operand. It does not by itself start a thread or guarantee parallel execution.", src.norm(operand)),
        definition: None,
        borrow: false,
    }))
}
