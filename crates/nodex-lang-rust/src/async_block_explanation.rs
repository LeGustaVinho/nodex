//! Beginner explanation of an `async { ... }` expression.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

pub(crate) fn explain<'t>(node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "async_block" || node.has_error() { return None; }
    let keyword = children(node).into_iter().find(|child| child.kind() == "async" && !child.is_missing())?;
    Some((keyword, ReadingContent::Detail {
        label: "Async block creates a future".into(),
        text: "Evaluating this async block constructs a future: a value representing work that can produce a result later. Creating the future does not run its body. The body progresses when the future is polled, for example while an enclosing running async task awaits it; an executor or another caller must drive the outer future. Polling may report that it is pending and later resume. If the block finishes normally, its final expression is the future's result; a block with no final value produces () (unit). Dropping the future prevents it from being polled further, although effects already performed are not undone. Captures follow Rust's borrowing and ownership rules; `async move` requests capture by value.".into(),
        definition: None, borrow: false,
    }))
}
