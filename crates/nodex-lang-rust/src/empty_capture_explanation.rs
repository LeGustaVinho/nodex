//! Explain empty precise capture for a return opaque type with an elided string reference input.
//! Rust Reference: https://doc.rust-lang.org/reference/types/impl-trait.html#precise-capturing
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") || function.has_error() { return Vec::new(); }
    let Some(parameters) = function.child_by_field_name("parameters") else { return Vec::new(); };
    let input = named_children(parameters).into_iter().find(|param| {
        if param.kind() != "parameter" { return false; }
        let Some(ty) = param.child_by_field_name("type") else { return false; };
        ty.kind() == "reference_type" && src.norm(ty) == "&str"
    });
    let Some(input) = input else { return Vec::new(); };
    let Some(result) = function.child_by_field_name("return_type") else { return Vec::new(); };
    if !matches!(result.kind(), "abstract_type" | "bounded_type") { return Vec::new(); }
    let mut stack = vec![result];
    let mut has_opaque = false;
    let mut empty_bounds = Vec::new();
    while let Some(part) = stack.pop() {
        if part.kind() == "abstract_type" {
            has_opaque = true;
            if let Some(bounds) = part.child_by_field_name("trait") { stack.push(bounds); }
        } else if part.kind() == "use_bounds" && named_children(part).into_iter().all(|child| child.is_extra()) {
            empty_bounds.push(part);
        } else if part.kind() == "bounded_type" {
            stack.extend(named_children(part).into_iter().rev());
        }
    }
    if !has_opaque { return Vec::new(); }
    empty_bounds.into_iter().map(|part| ContractItem {
                group: "Empty precise capture".into(),
                text: format!("`use<>` is an empty precise-capture list on this return-position `impl Trait`. It permits the hidden concrete return type to capture no in-scope generic parameters, including the elided lifetime of the written reference parameter `{}`. The function can still read that input while it runs, but its hidden return type cannot depend on the input's particular lifetime through capture. This is a type-level restriction, not a runtime closure capture, and the returned value may still contain owned data or references with independently valid lifetimes. Rust currently requires in-scope type and const parameters, and lifetimes required by other bounds, to be included; the source alone does not prove that this empty list or function body type-checks.", src.norm(input)),
                span: src.span(part), type_info: None,
            }).collect()
}
