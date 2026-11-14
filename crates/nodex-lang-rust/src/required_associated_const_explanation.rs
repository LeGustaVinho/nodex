//! Explain constants required by a trait, independently of ordinary constants.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>, trait_name: &str) -> Option<ContractItem> {
    if node.kind() != "const_item" || node.child_by_field_name("value").is_some() { return None; }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    Some(ContractItem {
        group: "Required associated constant".into(),
        text: format!("The trait {trait_name} requires each implementation to define the associated constant {} with type {}. This declaration names the constant and its type but provides no value; an implementation supplies its own constant value. `const` is a constant, and the colon introduces its type. It is associated with the implementing type rather than stored separately in each instance.", src.norm(name), src.norm(ty)),
        span: src.span(node),
        type_info: None,
    })
}

/// Explain on both the trait and the associated constant's own symbol.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "trait_item" {
        let Some(name) = node.child_by_field_name("name") else { return Vec::new() };
        let Some(body) = node.child_by_field_name("body") else { return Vec::new() };
        let trait_name = src.norm(name);
        return named_children(body).into_iter().filter_map(|item| describe(src, item, &trait_name)).collect();
    }
    if node.kind() == "const_item" {
        let Some(owner) = node.parent().and_then(|body| body.parent()).filter(|owner| owner.kind() == "trait_item") else { return Vec::new() };
        let Some(name) = owner.child_by_field_name("name") else { return Vec::new() };
        return describe(src, node, &src.norm(name)).into_iter().collect();
    }
    Vec::new()
}
