//! Explain outer block documentation attached to the following Rust item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, item: Node<'_>) -> Vec<ContractItem> {
    if !matches!(item.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_item" | "enum_variant" | "trait_item" | "type_item" | "associated_type" | "field_declaration" | "const_item" | "static_item" | "mod_item" | "macro_definition") { return Vec::new(); }
    let mut previous = item.prev_sibling();
    let mut result = Vec::new();
    while let Some(sibling) = previous {
        match sibling.kind() {
            "attribute_item" => {},
            "line_comment" => {
                let text = src.text(sibling);
                if !text.starts_with("///") || text.starts_with("////") { break; }
            }
            "block_comment" => {
                let text = src.text(sibling);
                if !text.starts_with("/**") || text.starts_with("/***") || text == "/**/" { break; }
                result.push(ContractItem {
                    group: "Outer documentation comment".into(),
                    text: "`/** ... */` is an outer documentation comment: it documents the item that follows it. Rust treats its contents as a documentation attribute, and tools such as rustdoc can use it to generate documentation. The comment describes the item for readers; it is not a runtime operation. The related `/*! ... */` form documents the enclosing item instead.".into(),
                    span: src.span(sibling), type_info: None,
                });
            }
            _ => break,
        }
        previous = sibling.prev_sibling();
    }
    result.reverse();
    result
}
