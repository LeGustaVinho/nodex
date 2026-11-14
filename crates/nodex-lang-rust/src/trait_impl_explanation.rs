//! Beginner explanation of implementing a trait for a type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" { return None; }
    let trait_type = implementation.child_by_field_name("trait")?;
    let concrete = implementation.child_by_field_name("type")?;
    // Negative impls declare that a trait is not implemented, and need a different explanation.
    if children(implementation).iter().any(|child| child.kind() == "!") { return None; }
    let trait_name = src.norm(trait_type);
    let type_name = src.norm(concrete);
    Some(ContractItem {
        group: "Trait implementation".into(),
        text: format!("This `impl {trait_name} for {type_name}` says that {type_name} implements the {trait_name} trait. A trait describes a set of behavior or associated items; this block supplies the required items that lack defaults and may override provided defaults for this type. Code can then use {type_name} where {trait_name} is required, subject to the trait's bounds and visibility. The declarations inside the braces are associated with this implementation; the block does not create a {type_name} value."),
        span: src.header_span(implementation, implementation.child_by_field_name("body")),
        type_info: None,
    })
}

/// Type declarations get same-module exact-name implementations; members get their enclosing impl.
/// No cross-file or name-resolution guesses are made.
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
