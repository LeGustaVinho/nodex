//! Explain lifetime-to-lifetime bounds in generic declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain bounds in an item's own signature, including where clauses.
pub(crate) fn contract(src: Src<'_>, item: Node<'_>) -> Vec<ContractItem> {
    if !matches!(item.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item" | "type_item" | "union_item") {
        return Vec::new();
    }
    let mut todo = vec![item];
    let mut result = Vec::new();
    while let Some(node) = todo.pop() {
        if node != item && matches!(node.kind(), "block" | "declaration_list" | "field_declaration_list" | "ordered_field_declaration_list" | "enum_variant_list" | "token_tree") { continue; }
        let subject = match node.kind() {
            "lifetime_parameter" => node.child_by_field_name("name"),
            "where_predicate" => node.child_by_field_name("left").filter(|n| n.kind() == "lifetime"),
            _ => None,
        };
        if let (Some(subject), Some(bounds)) = (subject, node.child_by_field_name("bounds")) {
            for bound in named_children(bounds).into_iter().filter(|bound| bound.kind() == "lifetime") {
                let longer = src.norm(subject);
                let shorter = src.norm(bound);
                result.push(ContractItem {
                    group: "Lifetime outlives bound".into(),
                    text: format!("{longer}: {shorter} requires lifetime {longer} to last at least as long as {shorter}. Any reference valid for {longer} may be used where {shorter} is required, subject to its type and borrowing rules. This is a compile-time relationship between lifetimes; it does not keep a value alive or extend its actual lifetime."),
                    span: src.span(node),
                    type_info: None,
                });
            }
        }
        todo.extend(named_children(node).into_iter().rev());
    }
    result
}
