//! Explain an `#[automatically_derived]` marker attached to an implementation.
//! Rust Reference: https://doc.rust-lang.org/reference/attributes/derive.html#the-automatically_derived-attribute
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn impl_markers(src: Src<'_>, implementation: Node<'_>) -> Vec<ContractItem> {
    if implementation.kind() != "impl_item" { return Vec::new(); }
    let mut previous = implementation.prev_named_sibling();
    let mut result = Vec::new();
    while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
        previous = attribute.prev_named_sibling();
        if attribute.kind() != "attribute_item" || attribute.has_error() { continue; }
        let Some(meta) = named_children(attribute).into_iter().find(|part| part.kind() == "attribute") else { continue; };
        let Some(name) = named_children(meta).into_iter().find(|part| part.kind() == "identifier") else { continue; };
        if src.text(name) != "automatically_derived" || meta.child_by_field_name("arguments").is_some() { continue; }
        let Some(target) = implementation.child_by_field_name("type") else { continue; };
        result.push(ContractItem {
            group: "Automatically derived implementation".into(),
            text: format!("#[automatically_derived] marks this implementation for {} as automatically created by a derive macro. Rust's built-in derives place this marker on generated implementations. The marker has no direct effect on the implementation's behavior; tools and diagnostic lints may use it to recognize generated code. The written marker indicates an origin, but by itself does not prove which macro produced this block. It applies to the following impl, not to the whole type or unrelated implementations.", src.norm(target)),
            span: src.span(attribute),
            type_info: None,
        });
    }
    result.reverse();
    result
}

/// Attach to members and placeholders, and to a same-module, exactly named type.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return impl_markers(src, node); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item") {
        return impl_markers(src, implementation);
    }
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name").map(|part| src.norm(part)) else { return Vec::new(); };
    let Some(container) = node.parent() else { return Vec::new(); };
    named_children(container).into_iter().filter(|candidate| candidate.kind() == "impl_item")
        .filter(|candidate| candidate.child_by_field_name("type").is_some_and(|ty| src.norm(ty) == name))
        .flat_map(|candidate| impl_markers(src, candidate)).collect()
}
