//! Explain the `FnOnce(...)` callable-trait bound without assuming name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "trait_item" | "impl_item" | "struct_item" | "type_item" | "associated_type") { return Vec::new(); }
    let body = declaration.child_by_field_name("body");
    let mut pending = vec![declaration];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body || node.kind() == "token_tree" { continue; }
        if node.kind() == "function_type"
            && let Some(trait_name) = node.child_by_field_name("trait")
        {
            let name = src.norm(trait_name);
            if matches!(name.as_str(), "FnOnce" | "std::ops::FnOnce" | "core::ops::FnOnce") {
                let args = node.child_by_field_name("parameters").map(|part| src.norm(part)).unwrap_or_else(|| "(...)".into());
                let output = node.child_by_field_name("return_type").map(|part| src.norm(part)).unwrap_or_else(|| "()".into());
                result.push(ContractItem {
                    group: "FnOnce callable bound".into(),
                    text: format!("{} describes a callable with input types {} and output type {}. If {} resolves to Rust's standard FnOnce trait, calling it takes the callable itself by value, so code with only this bound can invoke a particular value once; its implementation may consume captured data. It does not mean every call consumes captured data: callables that also implement FnMut or Fn can support repeated calls when used with the corresponding bound. The written name alone does not establish which trait it resolves to.", src.norm(node), args, output, name),
                    span: src.span(node), type_info: None,
                });
            }
        }
        pending.extend(named_children(node).into_iter().rev());
    }
    result
}
