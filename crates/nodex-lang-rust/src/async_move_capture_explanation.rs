//! Explain `async move { ... }` without assuming capture types implement Copy.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

pub(crate) fn explain<'t>(node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "async_block" || node.has_error() { return None; }
    let keyword = children(node).into_iter().find(|part| part.kind() == "move" && !part.is_missing())?;
    Some((keyword, ReadingContent::Detail {
        label: "Capture values in an async future".into(),
        text: "`async move` creates a future immediately and captures the values it uses from its surroundings by value. Each captured value is copied if its type implements Copy; otherwise it is moved, so the original binding may no longer be usable. If a captured value is itself a reference, capturing it does not transfer ownership of its referent and the reference must remain valid. The block body progresses only when the future is polled; creating it does not run the body or start a task. The future may be dropped without ever being polled.".into(),
        definition: None,
        borrow: false,
    }))
}
