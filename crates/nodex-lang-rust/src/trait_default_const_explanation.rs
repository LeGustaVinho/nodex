//! Explain a trait associated constant that supplies a default value.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>, trait_name: &str) -> Option<ContractItem> {
    if node.kind() != "const_item" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    let value = node.child_by_field_name("value")?;
    Some(ContractItem {
        group: "Default associated constant".into(),
        text: format!("The trait {trait_name} declares an associated constant {} of type {} and supplies the default expression {}. An implementation can omit this constant and use that default, or define its own value to override it. The constant belongs to the trait implementation for a type, rather than being stored separately in each instance. This syntax alone does not determine which implementation is used at a particular access.", src.norm(name), src.norm(ty), src.norm(value)),
        span: src.span(node), type_info: None,
    })
}

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "trait_item" {
        let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
        let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
        return named_children(body).into_iter().filter_map(|item| describe(src, item, &src.norm(name))).collect();
    }
    if node.kind() == "const_item" {
        let Some(owner) = node.parent().and_then(|body| body.parent()).filter(|owner| owner.kind() == "trait_item") else { return Vec::new(); };
        let Some(name) = owner.child_by_field_name("name") else { return Vec::new(); };
        return describe(src, node, &src.norm(name)).into_iter().collect();
    }
    Vec::new()
}
