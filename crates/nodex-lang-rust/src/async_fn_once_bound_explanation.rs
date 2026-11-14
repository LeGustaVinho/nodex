//! Explain inline `AsyncFnOnce` bounds without assuming name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, function: Node<'_>) -> Vec<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(parameters) = function.child_by_field_name("type_parameters") else { return Vec::new(); };
    let mut output = Vec::new();
    for parameter in named_children(parameters) {
        if parameter.kind() != "type_parameter" { continue; }
        let (Some(name), Some(bounds)) = (parameter.child_by_field_name("name"), parameter.child_by_field_name("bounds")) else { continue; };
        for bound in named_children(bounds) {
            if bound.kind() != "function_type" { continue; }
            let Some(trait_name) = bound.child_by_field_name("trait") else { continue; };
            if src.norm(trait_name) != "AsyncFnOnce" { continue; }
            let Some(args) = bound.child_by_field_name("parameters") else { continue; };
            let return_text = bound.child_by_field_name("return_type").map(|result| format!(" and specifies output type {}", src.norm(result))).unwrap_or_default();
            output.push(ContractItem {
                group: "Async callable bound".into(),
                text: format!("`{}: {}` requires the type parameter {} to satisfy the callable trait written on the right. The types {} are the callable's input types{}. If `AsyncFnOnce` resolves to Rust's standard trait, the callable can be invoked through this bound at most once: invoking it consumes the callable value and produces a future, whose asynchronous work progresses when polled. The callable may implement stronger traits that allow further calls in another context; its captured values are not necessarily all consumed. The name alone does not prove which trait it denotes.", src.norm(name), src.norm(bound), src.norm(name), src.norm(args), return_text),
                span: src.span(bound), type_info: None,
            });
        }
    }
    output
}
