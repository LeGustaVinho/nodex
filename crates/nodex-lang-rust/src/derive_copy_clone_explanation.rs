//! Explain deriving both Copy and Clone without assuming macro resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "struct_item" | "enum_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "derive" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")" | ",")).collect();
        if !parts.iter().any(|part| part.kind() == "identifier" && src.text(*part) == "Copy")
            || !parts.iter().any(|part| part.kind() == "identifier" && src.text(*part) == "Clone") { continue; }
        items.push(ContractItem {
            group: "Derived Copy and Clone".into(),
            text: "If Copy and Clone resolve to Rust's standard derive macros, this attribute asks the compiler to generate both trait implementations for the annotated type. Copy lets a by-value use copy the value's bits instead of moving the original; copying a reference or pointer does not duplicate its target. The derived Clone implementation explicitly clones the fields when clone is called. Copy requires Clone, all fields to be Copy, and the type not to implement Drop; generated implementations can also impose generic bounds. The written names can resolve differently, so the spelling alone does not prove these are the standard derives.".into(),
            span: src.span(attribute), type_info: None,
        });
    }
    items.reverse();
    items
}
