//! Explain an explicit panic-handler attribute without assuming build configuration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let name = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "this function".into());
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && named_children(attribute).into_iter().next().is_some_and(|path| src.norm(path) == "panic_handler")
                && attribute.child_by_field_name("arguments").is_none()
                && attribute.child_by_field_name("value").is_none()
            {
                result.push(ContractItem {
                    group: "Panic handler".into(),
                    text: format!("`#[panic_handler]` designates {name} as the function defining panic behavior. Rust requires the signature `fn(&PanicInfo) -> !`: the reference gives information about the panic, and `!` means the handler cannot return normally. A final program that needs a panic handler must have exactly one in its dependency graph; the standard library normally supplies one, while `no_std` final artifacts may need a custom handler. The handler body and panic strategy determine what happens, such as halting, aborting, or unwinding where supported. The attribute alone does not prove this signature, uniqueness, build configuration, or behavior is valid."),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
