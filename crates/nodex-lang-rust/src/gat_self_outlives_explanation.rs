//! Explain lifetime-parameterized associated types requiring `Self: 'a`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "associated_type" || node.has_error() { return Vec::new(); }
    if !node.parent().and_then(|body| body.parent()).is_some_and(|owner| owner.kind() == "trait_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let Some(parameters) = node.child_by_field_name("type_parameters") else { return Vec::new(); };
    let lifetime_names: Vec<_> = named_children(parameters).into_iter().filter(|parameter| parameter.kind() == "lifetime_parameter")
        .filter_map(|parameter| parameter.child_by_field_name("name"))
        .map(|lifetime| src.norm(lifetime)).collect();
    let Some(clause) = named_children(node).into_iter().find(|part| part.kind() == "where_clause") else { return Vec::new(); };
    let mut items = Vec::new();
    for predicate in named_children(clause).into_iter().filter(|part| part.kind() == "where_predicate") {
        let (Some(left), Some(bounds)) = (predicate.child_by_field_name("left"), predicate.child_by_field_name("bounds")) else { continue; };
        if src.norm(left) != "Self" { continue; }
        for lifetime in named_children(bounds).into_iter().filter(|part| part.kind() == "lifetime") {
            let written = src.norm(lifetime);
            if !lifetime_names.contains(&written) { continue; }
            items.push(ContractItem {
                group: "Associated type lifetime requirement".into(),
                text: format!("{}{} declares an associated type family parameterized by a lifetime; each implementation supplies the associated type, and {} is chosen when referring to a member of that family. The condition Self: {} requires the implementing type to outlive that chosen lifetime: references contained in Self must remain valid for at least {}. The condition must hold where this associated type is used. It constrains types and lifetimes; it does not keep any particular value alive or extend a borrow. A lifetime is a compile-time validity relationship, not a runtime value or allocation.", src.norm(name), src.norm(parameters), written, written, written),
                span: src.span(predicate), type_info: None,
            });
        }
    }
    items
}
