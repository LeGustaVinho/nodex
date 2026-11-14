use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Explain the function declaration itself, including for an empty body.
pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let keyword = children(node).into_iter().find(|child| child.kind() == "fn")?;
    let asynchronous = children(node).into_iter().filter(|child| child.kind() == "function_modifiers")
        .any(|modifiers| children(modifiers).iter().any(|child| child.kind() == "async"));
    let execution = if asynchronous {
        "Calling it creates a future; the body progresses when that future is polled."
    } else {
        "The body runs when the function is called."
    };
    Some(ContractItem {
        group: "Function".into(),
        text: format!(
            "fn declares a function named {}. The parentheses contain its parameters, and the braces contain its body. Declaring it makes the function available. {}",
            src.norm(name), execution,
        ),
        span: src.span(keyword),
        type_info: None,
    })
}
