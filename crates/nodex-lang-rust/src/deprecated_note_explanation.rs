//! Explain a `#[deprecated(note = ...)]` attribute on its annotated item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if matches!(node.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "deprecated" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        for (index, group) in parts.windows(3).enumerate() {
            let [key, equal, value] = group else { continue; };
            if src.text(*key) != "note" || equal.kind() != "=" || !matches!(value.kind(), "string_literal" | "raw_string_literal") { continue; }
            if index > 0 && parts[index - 1].kind() != "," { continue; }
            if index + 3 < parts.len() && parts[index + 3].kind() != "," { continue; }
            result.push(ContractItem {
                group: "Deprecation notice".into(),
                text: format!("{} marks the following item as deprecated: its author is discouraging future use, often in favor of a replacement. The note is written as {} and can be shown with a compiler deprecation diagnostic at a use site. Deprecation normally produces a warning rather than removing the item; lint settings and the use context can change which diagnostic is emitted. This attribute does not itself call the replacement or alter the item's runtime behavior.", src.norm(attribute), src.text(*value)),
                span: src.span(attribute), type_info: None,
            });
            break;
        }
    }
    result.reverse();
    result
}
