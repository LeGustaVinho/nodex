//! Explain the spelling of the standard addition trait without assuming name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" || children(implementation).iter().any(|n| n.kind() == "!") { return None; }
    let written_trait = implementation.child_by_field_name("trait")?;
    let trait_path = if written_trait.kind() == "generic_type" {
        written_trait.child_by_field_name("type")?
    } else { written_trait };
    let path: String = src.text(trait_path).chars().filter(|c| !c.is_whitespace()).collect();
    if !matches!(path.as_str(), "std::ops::Add" | "::std::ops::Add") { return None; }
    let target = implementation.child_by_field_name("type")?;
    let text = format!("This block implements {} for {}. If the path resolves to Rust's standard Add trait, it defines how the + operator works with {} on the left: x + y calls the trait's add method with x and y. The right-hand operand type is the trait's generic argument, or Self when omitted; the associated Output type determines the result type. Operand ownership depends on the implemented types and method signature. The path must resolve to that standard trait for these operator rules to apply.", src.norm(written_trait), src.norm(target), src.norm(target));
    Some(ContractItem { group: "Addition operator implementation".into(), text, span: src.header_span(implementation, implementation.child_by_field_name("body")), type_info: None })
}

/// Attach to an impl placeholder, its members, or an exact same-module type declaration.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return describe(src, node).into_iter().collect(); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|p| p.kind() == "impl_item") {
        return describe(src, implementation).into_iter().collect();
    }
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name").map(|n| src.norm(n)) else { return Vec::new(); };
    let Some(container) = node.parent() else { return Vec::new(); };
    named_children(container).into_iter()
        .filter(|candidate| candidate.kind() == "impl_item")
        .filter(|candidate| candidate.child_by_field_name("type").is_some_and(|ty| src.norm(ty) == name))
        .filter_map(|candidate| describe(src, candidate)).collect()
}
