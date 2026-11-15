//! Explain a trait implementation for a generic four-element array.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" || implementation.has_error() || children(implementation).iter().any(|part| part.kind() == "!") { return None; }
    let trait_name = implementation.child_by_field_name("trait")?;
    let array = implementation.child_by_field_name("type")?;
    if array.kind() != "array_type" { return None; }
    let element = array.child_by_field_name("element")?;
    let length = array.child_by_field_name("length")?;
    if element.kind() != "type_identifier" || length.kind() != "integer_literal" || src.text(length) != "4" { return None; }
    let parameters = implementation.child_by_field_name("type_parameters")?;
    if !named_children(parameters).iter().any(|parameter| parameter.kind() == "type_parameter"
        && parameter.child_by_field_name("name").is_some_and(|name| src.norm(name) == src.norm(element))) { return None; }
    let element_name = src.norm(element);
    let trait_text = src.norm(trait_name);
    Some(ContractItem {
        group: "Trait implementation for a four-element array".into(),
        text: format!("`impl` declares an implementation of {trait_text} for the array type {}. The parameter {element_name} stands for an element type, so this can cover a family of arrays with different element types that satisfy the written and implicit bounds. The `4` is the array length and part of its type: this implementation does not by itself cover a different length or a slice `[{element_name}]`. The braces contain the implementation's associated definitions and may be empty when none are required. Rust must still resolve the trait and check bounds, orphan rules, and overlapping implementations; this syntax alone does not prove the implementation is valid.", src.norm(array)),
        span: src.header_span(implementation, implementation.child_by_field_name("body")), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return describe(src, node).into_iter().collect(); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item") {
        return describe(src, implementation).into_iter().collect();
    }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    container.map(|container| named_children(container).into_iter().filter_map(|item| describe(src, item)).collect()).unwrap_or_default()
}
