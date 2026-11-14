use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Explain the function declaration itself, including for an empty body.
pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let keyword = children(node).into_iter().find(|child| child.kind() == "fn")?;
    Some(ContractItem {
        group: "Function".into(),
        text: format!(
            "fn declares a function named {}. The parentheses contain its parameters, and the braces contain its body. Declaring it makes the function available; the body runs when the function is called.",
            src.norm(name),
        ),
        span: src.span(keyword),
        type_info: None,
    })
}
