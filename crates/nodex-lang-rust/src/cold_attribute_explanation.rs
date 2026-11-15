//! Explain the `#[cold]` optimization hint on functions with bodies.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() { return Vec::new(); }
    let name = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "this function".into());
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if let Some(attribute) = named_children(part).into_iter().find(|child| child.kind() == "attribute")
                && named_children(attribute).into_iter().next().is_some_and(|path| src.norm(path) == "cold")
                && attribute.child_by_field_name("arguments").is_none()
                && attribute.child_by_field_name("value").is_none()
            {
                result.push(ContractItem {
                    group: "Infrequent function hint".into(),
                    text: format!("`#[cold]` tells the compiler that calls to {name} are expected to be uncommon, for example on a rare error path. It is an optimization hint: the compiler may use it when choosing generated code or layout, but it does not guarantee a particular optimization or execution frequency. The function can still be called normally; the attribute does not prevent execution or change what the function means. Rust specifies this form for functions with bodies, and the actual effect depends on the compiler and build options."),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
