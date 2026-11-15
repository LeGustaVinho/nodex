//! Explain forward-compatible `non_exhaustive` struct declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_non_exhaustive(src: Src<'_>, item: Node<'_>) -> bool {
    if item.kind() != "attribute_item" || item.has_error() { return false; }
    let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return false; };
    let parts: Vec<_> = children(attr).into_iter().filter(|part| !part.is_extra()).collect();
    matches!(parts.as_slice(), [name] if name.kind() == "identifier" && src.text(*name) == "non_exhaustive")
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "struct_item" { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if is_non_exhaustive(src, attribute) {
            result.push(ContractItem {
                group: "Non-exhaustive struct".into(),
                text: "#[non_exhaustive] marks this struct as open to future fields, helping its defining crate evolve without requiring external users to list every field. Inside the defining crate, ordinary construction and matching remain available subject to normal visibility rules. Outside the defining crate, code cannot construct it with a struct expression, including struct update syntax; tuple and unit struct constructors are not public outside the crate. Obtain a value through an API supplied by the defining crate, if available. External destructuring must use a braced struct pattern containing `..` (for example `Config { field, .. }`), including for tuple or unit structs. The attribute does not itself provide a constructor, add fields, or prove any API is available.".into(),
                span: src.span(attribute), type_info: None,
            });
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
