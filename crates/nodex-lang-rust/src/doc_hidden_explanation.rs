//! Explain the `#[doc(hidden)]` outer documentation attribute.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        if attribute.kind() == "attribute_item" {
            let written: String = src.text(attribute).chars().filter(|ch| !ch.is_whitespace()).collect();
            if written == "#[doc(hidden)]" {
                result.push(ContractItem {
                    group: "Hidden from generated documentation".into(),
                    text: "`#[doc(hidden)]` asks rustdoc to omit the annotated item from ordinary generated API documentation. It changes documentation presentation, not the item's Rust visibility: a `pub` item can still be accessible to code according to normal visibility rules. It does not remove the item from compilation or make it secret. The actual generated documentation also depends on rustdoc settings and build configuration.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
