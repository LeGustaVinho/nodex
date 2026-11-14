//! Explain an `async move |...|` closure without guessing capture types.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(_src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "closure_expression" || node.has_error() { return None; }
    let tokens = children(node);
    if !tokens.iter().any(|token| token.kind() == "async") { return None; }
    let keyword = tokens.into_iter().find(|token| token.kind() == "move")?;
    Some((keyword, ReadingContent::Detail {
        label: "Create an async move closure".into(),
        text: "This expression creates a closure (a callable value); the code between the vertical bars names its parameters. `move` captures the outer values it uses by value into the closure: a captured value is copied if its type implements Copy, otherwise it is moved. The syntax alone does not determine which applies. Calling the closure returns a future; the async body progresses when that future is polled, such as through await, and can suspend. Creating or calling the closure does not by itself schedule a background task. Whether a returned future borrows from the closure's captures depends on how the closure is called and used.".into(),
        definition: None,
        borrow: false,
    }))
}
