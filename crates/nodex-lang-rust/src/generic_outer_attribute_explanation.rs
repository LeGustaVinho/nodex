//! Explain outer attributes on declarations without assuming their identity.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

/// Walk adjacent outer attributes and comments belonging to this item only.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_item" | "enum_variant" | "trait_item" | "type_item" | "associated_type" | "field_declaration" | "const_item" | "static_item" | "mod_item" | "macro_definition") { return Vec::new(); }
    let mut previous = node.prev_sibling();
    let mut result = Vec::new();
    while let Some(part) = previous {
        match part.kind() {
            "attribute_item" if !part.has_error() => result.push(ContractItem {
                group: "Outer attribute".into(),
                text: format!("{} is an outer attribute: `#[...]` attaches attribute information to the declaration that follows it. Depending on the resolved attribute and context, it can supply metadata or affect compilation; an attribute macro can also transform the annotated syntax. The written name alone does not establish its meaning, whether it is accepted here, or a runtime action. Rust checks the attribute and its placement; `#![...]` instead applies to an enclosing item such as a module or crate.", src.norm(part)),
                span: src.span(part),
                type_info: None,
            }),
            "line_comment" if !src.text(part).starts_with("//!") => {},
            "block_comment" if !src.text(part).starts_with("/*!") => {},
            _ => break,
        }
        previous = part.prev_sibling();
    }
    result.reverse();
    result
}
