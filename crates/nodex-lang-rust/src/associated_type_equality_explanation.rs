//! Explain equality constraints on a trait's associated type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain written trait-associated type equalities in a symbol's own header.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() == "source_file" { return Vec::new(); }
    let body = declaration.child_by_field_name("body");
    let mut pending: Vec<_> = named_children(declaration).into_iter().rev().filter(|n| Some(*n) != body).collect();
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node.is_extra() || matches!(node.kind(), "block" | "declaration_list" | "token_tree") { continue; }
        if node.kind() == "type_binding"
            && let Some(trait_path) = node.parent().and_then(|args| args.parent())
                .filter(|parent| parent.kind() == "generic_type")
                .and_then(|generic| generic.child_by_field_name("type"))
            && let Some(name) = node.child_by_field_name("name")
            && let Some(ty) = node.child_by_field_name("type")
        {
            let trait_name = src.norm(trait_path);
            let associated = src.norm(name);
            let expected = src.norm(ty);
            let iterator_hint = if trait_name == "Iterator" || trait_name.ends_with("::Iterator") {
                " If Iterator resolves to Rust's standard iterator trait, Item is the type of each item it yields; this constraint says that yielded type is the specified type. The spelling alone does not prove which trait is meant."
            } else { "" };
            items.push(ContractItem {
                group: "Associated type equality".into(),
                text: format!("In {trait_name}<{associated} = {expected}>, {associated} is an associated type chosen by the implementation of {trait_name}. The `=` requires that chosen type to be exactly {expected}; it does not assign a runtime value. The bound applies to types satisfying this trait requirement.{iterator_hint}"),
                span: src.span(node),
                type_info: None,
            });
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
