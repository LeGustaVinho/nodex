//! Explain a written `#[custom_attribute]` without pretending it is resolved.
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
            if written == "#[custom_attribute]" {
                result.push(ContractItem {
                    group: "Custom outer attribute".into(),
                    text: "`#[custom_attribute]` is an outer attribute attached to the following declaration. If this name resolves to a procedural attribute macro, the compiler invokes that macro during compilation with the attribute's input (empty here) and the annotated item's tokens; the macro can produce replacement syntax. The spelling alone does not prove that resolution: the attribute could instead be a derive helper or be invalid in this context. This does not by itself describe runtime execution or C#-style attribute metadata.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
