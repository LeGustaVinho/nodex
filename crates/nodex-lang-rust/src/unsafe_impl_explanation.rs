//! Explain the safety promise made by an `unsafe impl` of a trait.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn item(src: Src<'_>, implementation: Node<'_>) -> Option<ContractItem> {
    if implementation.kind() != "impl_item" { return None; }
    let keyword = children(implementation).into_iter().find(|child| child.kind() == "unsafe")?;
    let trait_name = implementation.child_by_field_name("trait")?;
    let target = implementation.child_by_field_name("type")?;
    if children(implementation).iter().any(|child| child.kind() == "!") { return None; }
    Some(ContractItem {
        group: "Unsafe trait implementation".into(),
        text: format!("`unsafe impl {} for {}` implements the trait for this type and asserts that its implementation upholds the trait's safety requirements. The author must check the trait documentation and preserve every required invariant; the compiler does not prove those obligations. Other unsafe code may rely on this promise. The block can be empty if no associated items need to be supplied; empty braces do not remove the safety obligation. `unsafe` here qualifies the implementation and does not itself run the code inside the block.", src.norm(trait_name), src.norm(target)),
        span: src.span(keyword),
        type_info: None,
    })
}

/// Explain an implementation on its owner or members, without guessing cross-file resolution.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "impl_item" { return item(src, node).into_iter().collect(); }
    if let Some(implementation) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "impl_item") {
        return item(src, implementation).into_iter().collect();
    }
    if !matches!(node.kind(), "struct_item" | "enum_item" | "union_item") { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name").map(|name| src.norm(name)) else { return Vec::new(); };
    let Some(container) = node.parent() else { return Vec::new(); };
    named_children(container).into_iter()
        .filter(|candidate| candidate.kind() == "impl_item")
        .filter(|candidate| candidate.child_by_field_name("type").is_some_and(|ty| src.norm(ty) == name))
        .filter_map(|candidate| item(src, candidate)).collect()
}
