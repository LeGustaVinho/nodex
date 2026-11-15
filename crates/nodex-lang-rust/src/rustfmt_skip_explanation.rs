//! Explain the `#[rustfmt::skip]` outer tool attribute on a Rust item.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn is_skip(src: Src<'_>, attribute: Node<'_>) -> bool {
    if attribute.kind() != "attribute_item" || attribute.has_error() { return false; }
    let Some(inner) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { return false };
    if inner.child_by_field_name("arguments").is_some() || inner.child_by_field_name("value").is_some() { return false; }
    let Some(path) = named_children(inner).into_iter().find(|part| part.kind() == "scoped_identifier") else { return false };
    let (Some(prefix), Some(name)) = (path.child_by_field_name("path"), path.child_by_field_name("name")) else { return false };
    prefix.kind() == "identifier" && name.kind() == "identifier" && src.text(prefix) == "rustfmt" && src.text(name) == "skip"
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if matches!(declaration.kind(), "source_file" | "attribute_item" | "inner_attribute_item") { return Vec::new(); }
    let mut result = Vec::new();
    let mut previous = declaration.prev_named_sibling();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if !is_skip(src, attribute) { continue; }
        result.push(ContractItem {
            group: "Skip rustfmt formatting".into(),
            text: "#[rustfmt::skip] is an outer tool attribute on the following item. It tells rustfmt, when that formatter is run, to leave the annotated item out of its automatic formatting. The rustfmt:: prefix identifies the formatting tool; this is not a Rust instruction executed at runtime, does not make the compiler skip the item, and does not disable unrelated tools or checks. Exact tool behavior can depend on the formatter version and invocation. This direct attribute differs from cfg_attr, which conditionally applies an attribute.".into(),
            span: src.span(attribute), type_info: None,
        });
    }
    result
}
