//! Explain an item's `#[cfg(any(...))]` attribute.
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
        if src.text(name) != "cfg" { continue; }
        let Some(args) = meta.child_by_field_name("arguments") else { continue; };
        let outer: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
        let [any, inner] = outer.as_slice() else { continue; };
        if src.text(*any) != "any" || inner.kind() != "token_tree" || !src.text(*inner).starts_with('(') { continue; }
        let empty = children(*inner).into_iter().all(|part| part.is_extra() || matches!(part.kind(), "(" | ")"));
        let rule = if empty {
            "any() has no predicates, so it is false for every compilation configuration."
        } else {
            "any(...) is true when at least one of its predicates is true for the compilation configuration; it is false when none are true. Its result depends on the configured target and options."
        };
        result.push(ContractItem {
            group: "Any configuration predicate".into(),
            text: format!("{} is a conditional-compilation attribute on the following item. {} If this condition is true, the attribute permits the item to be included; if false, the item is excluded from that build. Other attributes and conditions can still affect inclusion. This is a compile-time configuration decision, not a runtime if. The syntax alone does not establish which flags or target values are active.", src.norm(attribute), rule),
            span: src.span(attribute),
            type_info: None,
        });
    }
    result.reverse();
    result
}
