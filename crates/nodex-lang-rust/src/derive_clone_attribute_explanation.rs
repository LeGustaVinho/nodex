//! Explain deriving Clone on a struct or enum without assuming macro identity.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn contains_simple_clone(src: Src<'_>, item: Node<'_>) -> bool {
    let Some(attribute) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { return false; };
    let Some(path) = named_children(attribute).into_iter().next() else { return false; };
    if src.norm(path) != "derive" { return false; }
    let Some(args) = attribute.child_by_field_name("arguments") else { return false; };
    let tokens: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra()).collect();
    tokens.windows(3).any(|window| src.text(window[1]) == "Clone" && matches!(window[0].kind(), "(" | ",") && matches!(window[2].kind(), ")" | ","))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "struct_item" | "enum_item") { return Vec::new(); }
    let mut output = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            if contains_simple_clone(src, part) {
                output.push(ContractItem {
                    group: "Derive Clone".into(),
                    text: "`#[derive(Clone)]` requests generated code for the following type during compilation. If `Clone` resolves to Rust's standard derive macro, it generates an implementation of the standard `Clone` trait; cloning a struct or enum clones its fields, and generic types may need corresponding bounds. The fields must support the required cloning operations. `clone()` is an explicit operation, not an automatic copy on every assignment, and a field's clone may share data rather than making an independent deep copy. `Clone` could refer to a different derive macro, so its spelling alone does not prove that standard behavior.".into(),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    output.reverse();
    output
}
