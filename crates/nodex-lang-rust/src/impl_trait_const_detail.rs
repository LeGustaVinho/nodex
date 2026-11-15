//! Explain an associated constant supplied in a trait implementation.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "const_item" || node.has_error() { return None; }
    let trait_name = implementation.child_by_field_name("trait")?;
    let implementer = implementation.child_by_field_name("type")?;
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    let value = node.child_by_field_name("value")?;
    Some(ContractItem {
        group: "Trait implementation constant".into(),
        text: format!("In this implementation of {} for {}, `const {}: {} = {}` supplies the associated constant {} with the given type and constant expression for this implementation. It belongs to this trait implementation rather than being stored separately in every instance. Code can refer to it with `<{} as {}>::{}` when the names resolve as intended. The trait declaration determines whether this constant is required or overrides a default; the written syntax alone does not establish resolution or validity.", src.norm(trait_name), src.norm(implementer), src.norm(name), src.norm(ty), src.norm(value), src.norm(name), src.norm(implementer), src.norm(trait_name), src.norm(name)),
        span: src.span(node), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" {
        let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
        return named_children(body).into_iter().filter_map(|item| describe(src, item, node)).collect();
    }
    if node.kind() == "const_item" {
        let Some(owner) = node.parent().and_then(|p| p.parent()).filter(|p| p.kind() == "impl_item") else { return Vec::new(); };
        return describe(src, node, owner).into_iter().collect();
    }
    Vec::new()
}
