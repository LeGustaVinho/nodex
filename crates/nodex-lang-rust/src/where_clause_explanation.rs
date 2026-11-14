//! Explain a Rust `where` clause as a set of compile-time requirements.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn where_clause(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node).into_iter().find(|child| child.kind() == "where_clause")
}

fn describe(src: Src<'_>, clause: Node<'_>) -> ContractItem {
    let predicates: Vec<String> = named_children(clause).into_iter()
        .filter(|part| !part.is_extra()).map(|part| src.norm(part)).collect();
    let list = predicates.join(", ");
    ContractItem {
        group: "Where clause".into(),
        text: format!("`where` lists compile-time requirements on generic types or lifetimes. Each comma separates another requirement; all requirements must hold, not just one. Here the requirements are {list}. For example, `T: A` requires the type substituted for T to satisfy the bound A, and `U: B` imposes B on U. A bound can name a trait or lifetime; its precise meaning depends on what is written. The clause constrains which substitutions are valid; it does not run code or create values."),
        span: src.span(clause), type_info: None,
    }
}

/// Explain an item's clause and the enclosing impl/trait clause on members.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" | "impl_item" | "const_item" | "associated_type") { return Vec::new(); }
    let mut out = Vec::new();
    if let Some(clause) = where_clause(node) { out.push(describe(src, clause)); }
    if let Some(owner) = node.parent().and_then(|body| body.parent())
        .filter(|owner| matches!(owner.kind(), "impl_item" | "trait_item"))
        && let Some(clause) = where_clause(owner) {
        out.push(describe(src, clause));
    }
    out
}
