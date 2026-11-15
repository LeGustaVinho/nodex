//! Explain the bare `#[deprecated]` attribute on an item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_item" | "enum_variant" | "trait_item" | "type_item" | "associated_type" | "field_declaration" | "const_item" | "static_item" | "mod_item" | "macro_definition") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = node.prev_named_sibling();
    while let Some(part) = previous {
        if part.kind() == "attribute_item" {
            let compact: String = src.text(part).chars().filter(|ch| !ch.is_whitespace()).collect();
            if compact == "#[deprecated]" {
                result.push(ContractItem {
                    group: "Deprecated item".into(),
                    text: "`#[deprecated]` marks the following item as deprecated: its author signals that callers should move away from it. Use of a deprecated item can trigger Rust's deprecation lint, normally a warning; lint settings can change whether it is shown or treated as an error. The attribute does not itself remove the item or make a use impossible. This bare form gives no reason or replacement; other forms can supply additional details.".into(),
                    span: src.span(part), type_info: None,
                });
            }
        } else if !part.is_extra() { break; }
        previous = part.prev_named_sibling();
    }
    result.reverse();
    result
}
