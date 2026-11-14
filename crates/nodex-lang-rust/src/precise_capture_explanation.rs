//! Explain precise capture bounds on opaque return types.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Inspect only the declared return type, not nested bodies or unrelated `use` items.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") {
        return Vec::new();
    }
    let Some(result) = node.child_by_field_name("return_type") else { return Vec::new() };
    let mut pending = vec![result];
    let mut facts = Vec::new();
    while let Some(part) = pending.pop() {
        if part.kind() == "use_bounds" {
            let args: Vec<_> = named_children(part).into_iter().map(|arg| src.norm(arg)).collect();
            let names = if args.is_empty() {
                "The list is empty, so no generic parameters are listed for capture.".to_owned()
            } else {
                format!("The list names {} as parameters the hidden type may capture; generic parameters omitted from this list are excluded from capture.", args.join(", "))
            };
            facts.push(ContractItem {
                group: "Precise opaque type capture".into(),
                text: format!("{} specifies the generic parameters that the hidden concrete type behind this impl Trait return may depend on. {} Capture here is a type-level permission, not a runtime copy or move of values, and listing a lifetime does not itself keep data alive. Rust checks which parameters must be listed and whether the hidden type and other bounds are valid.", src.norm(part), names),
                span: src.span(part), type_info: None,
            });
            continue;
        }
        if part.kind() != "token_tree" { pending.extend(named_children(part).into_iter().rev()); }
    }
    facts
}
