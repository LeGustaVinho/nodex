//! Explain inherent implementation blocks on the type they extend.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, item: Node<'_>) -> Option<ContractItem> {
    if item.kind() != "impl_item" || item.child_by_field_name("trait").is_some() { return None; }
    let target = item.child_by_field_name("type")?;
    let body = item.child_by_field_name("body")?;
    let parameters = item.child_by_field_name("type_parameters")
        .map(|p| format!(" The parameters {} describe a family of types; any bounds restrict which types this implementation applies to.", src.norm(p)))
        .unwrap_or_default();
    Some(ContractItem {
        group: "Inherent implementation".into(),
        text: format!("`impl {}` adds methods and other associated items to the type {}. The braces contain their definitions; this block does not create a value. Methods with a `self` receiver operate on a value, while associated functions without one are called through the type, for example Player::new() for a Player implementation. The type and implementation can be declared separately.{}", src.norm(target), src.norm(target), parameters),
        span: src.header_span(item, Some(body)),
        type_info: None,
    })
}

/// Explain exact same-module implementations on a type, and the enclosing
/// implementation on its members. Avoid guessing cross-file resolution.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return item(src, node).into_iter().collect(); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent())
        .filter(|parent| parent.kind() == "impl_item") {
        return item(src, implementation).into_iter().collect();
    }
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name").map(|n| src.norm(n)) else { return Vec::new(); };
    let Some(container) = node.parent() else { return Vec::new(); };
    named_children(container).into_iter()
        .filter(|candidate| candidate.kind() == "impl_item")
        .filter(|candidate| candidate.child_by_field_name("type").is_some_and(|ty| src.norm(ty) == name))
        .filter_map(|candidate| item(src, candidate)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};
    #[test]
    fn distinguishes_inherent_from_trait_implementation() {
        let text = "struct Player; impl Player { fn new() -> Self { Player } } impl Draw for Player {}";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let items = named_children(tree.root_node());
        let result = item(src, items[1]).unwrap();
        assert!(result.text.contains("adds methods"));
        assert!(result.text.contains("does not create a value"));
        assert!(result.text.contains("Player::new()"));
        assert!(item(src, items[2]).is_none());
        assert_eq!(explain(src, items[0]).len(), 1);
    }
}
