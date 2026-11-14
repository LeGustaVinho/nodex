//! Explain the creation and execution timing of an async closure.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::children;
use tree_sitter::Node;

pub(crate) fn explain(node: Node<'_>) -> Option<(Node<'_>, ReadingContent)> {
    if node.kind() != "closure_expression" || node.has_error() { return None; }
    let parts = children(node);
    let keyword = parts.iter().copied().find(|part| part.kind() == "async")?;
    let moves = parts.iter().any(|part| part.kind() == "move");
    let capture = if moves {
        " The `move` keyword makes this closure capture the used outer values by value (copying values that are Copy)."
    } else {
        " It can capture values from its surrounding scope; the capture mode depends on how they are used."
    };
    Some((keyword, ReadingContent::Detail {
        label: "Create an async closure".into(),
        text: format!("This creates a callable closure; the names between `|` are its parameters. Calling it with arguments creates a future for that invocation rather than running the body to completion immediately. The body makes progress when that future is polled, can suspend at an `await`, and produces its result when complete. Creating the future does not automatically schedule it; it needs to be polled, for example by awaiting it or by an executor.{capture}"),
        definition: None,
        borrow: false,
    }))
}
