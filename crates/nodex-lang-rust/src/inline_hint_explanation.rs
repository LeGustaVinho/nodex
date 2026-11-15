//! Explain the plain `#[inline]` optimization hint on a function.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let mut previous = node.prev_named_sibling();
    let mut items = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "inline" || meta.child_by_field_name("arguments").is_some() || meta.child_by_field_name("value").is_some() { continue; }
        items.push(ContractItem {
            group: "Inlining hint".into(),
            text: "#[inline] is a hint that the compiler may consider placing this function's body at a call site instead of making a conventional call. It can help expose optimization opportunities, but the compiler can ignore the hint and it does not guarantee faster code or a particular machine-code shape. It does not change the function's Rust behavior. The plain form differs from #[inline(always)] and #[inline(never)], which express stronger and opposing inlining preferences.".into(),
            span: src.span(attribute), type_info: None,
        });
    }
    items.reverse();
    items
}
