//! Explain a trait implementation whose target is `&mut T`, for generic `T`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" || implementation.has_error() || children(implementation).iter().any(|part| part.kind() == "!") { return None; }
    let trait_type = implementation.child_by_field_name("trait")?;
    let target = implementation.child_by_field_name("type")?;
    if target.kind() != "reference_type" || !children(target).iter().any(|part| part.kind() == "mutable_specifier") { return None; }
    let inner = target.child_by_field_name("type")?;
    if inner.kind() != "type_identifier" { return None; }
    let params = implementation.child_by_field_name("type_parameters")?;
    if !named_children(params).iter().any(|param| param.kind() == "type_parameter" && param.child_by_field_name("name").is_some_and(|name| src.text(name) == src.text(inner))) { return None; }
    Some(ContractItem {
        group: "Trait implementation for mutable references".into(),
        text: format!("This `impl` supplies the trait {} for the mutable-reference type {}, with {} as a generic type parameter. It applies to eligible mutable references for types meeting the implementation's bounds and Rust's trait-coherence rules; it does not by itself implement the trait for {} itself or for shared references. `&mut` is an exclusive borrow of the underlying value rather than ownership of that value, and the reference must remain valid. What the trait methods actually do depends on their definitions; the implementation does not imply that every call mutates the value.", src.norm(trait_type), src.norm(target), src.norm(inner), src.norm(inner)),
        span: src.header_span(implementation, implementation.child_by_field_name("body")), type_info: None,
    })
}

/// Explain direct impls on their containing module, even if the impl is empty;
/// also expose the explanation on an impl placeholder or one of its members.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return item(src, node).into_iter().collect(); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item") {
        return item(src, implementation).into_iter().collect();
    }
    let container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new(); };
    named_children(container).into_iter().filter_map(|candidate| item(src, candidate)).collect()
}
