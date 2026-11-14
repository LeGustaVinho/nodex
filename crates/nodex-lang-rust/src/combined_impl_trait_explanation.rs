//! Explain combined bounds in an `impl Trait` type without assuming name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Explain only a declaration's own signature; nested bodies have separate symbols.
pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item") {
        return Vec::new();
    }
    let body = declaration.child_by_field_name("body");
    let result = declaration.child_by_field_name("return_type");
    let mut pending = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if node.kind() == "bounded_type"
            && named_children(node).first().is_some_and(|child| child.kind() == "abstract_type")
            && children(node).iter().any(|child| child.kind() == "+")
        {
            let context = if Some(node) == result {
                "In a return type, the function implementation chooses the concrete type; callers can rely on its listed bounds rather than its hidden identity. All return paths for this opaque return must resolve to the same concrete type."
            } else {
                "In a parameter, the caller supplies a value of a concrete type satisfying all of these bounds; the function can use the operations promised by them."
            };
            items.push(ContractItem {
                group: "Combined impl Trait bounds".into(),
                text: format!("In {}, impl introduces a concrete type described through its bounds. The + joins requirements: the same concrete type must satisfy every listed bound, not merely one. {} If Send refers to Rust's standard Send trait, that bound says values of the concrete type can be transferred between threads; the spelling alone does not prove which item a name resolves to. This describes a concrete type, while dyn Trait denotes a trait object with dynamic dispatch.", src.norm(node), context),
                span: src.span(node),
                type_info: None,
            });
            continue;
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    items
}
