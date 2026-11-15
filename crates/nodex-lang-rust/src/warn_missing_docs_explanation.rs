//! Explain the explicit `warn(missing_docs)` outer lint setting.
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
            if written == "#[warn(missing_docs)]" {
                result.push(ContractItem {
                    group: "Warn on missing documentation".into(),
                    text: "`#[warn(missing_docs)]` sets the `missing_docs` lint to the warning level for the annotated scope. The compiler can report missing documentation for public items where that lint applies; this attribute does not create documentation. It requests a warning rather than an error, but the final lint level and diagnostics can be affected by enclosing attributes, compiler options, visibility, and build configuration. The attribute alone does not prove that any item is missing documentation or that a warning will be emitted.".into(),
                    span: src.span(attribute), type_info: None,
                });
            }
        }
        previous = attribute.prev_named_sibling();
    }
    result
}
