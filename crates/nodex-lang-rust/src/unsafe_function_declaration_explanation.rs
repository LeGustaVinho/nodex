//! Explain the caller obligations of an explicitly unsafe function.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let modifiers = children(node).into_iter().find(|child| child.kind() == "function_modifiers")?;
    let keyword = children(modifiers).into_iter().find(|child| child.kind() == "unsafe")?;
    let name = node.child_by_field_name("name").map(|part| src.norm(part)).unwrap_or_else(|| "this function".into());
    Some(ContractItem {
        group: "Unsafe function".into(),
        text: format!("`unsafe fn {name}` declares a function whose caller must uphold additional safety requirements. Calling it requires an unsafe context; the caller must check the function's documented safety contract, which cannot be inferred from `unsafe` alone. The declaration does not itself call the function or turn off Rust's checks. Unsafe operations in its body still need their own justification; depending on the edition and lint settings they should be enclosed in explicit unsafe blocks."),
        span: src.span(keyword),
        type_info: None,
    })
}
