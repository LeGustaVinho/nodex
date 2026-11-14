//! Explain what an async function returns and when its body executes.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let modifiers = children(node).into_iter().find(|child| child.kind() == "function_modifiers")?;
    let keyword = children(modifiers).into_iter().find(|child| child.kind() == "async")?;
    let output = node.child_by_field_name("return_type").map(|ty| src.norm(ty)).unwrap_or_else(|| "()".into());
    Some(ContractItem {
        group: "Async function".into(),
        text: format!("Calling this async function evaluates its call arguments and returns a future; it does not by itself run the function body to completion or start a background task. The written return type {output} is the future's output when it completes, not the type immediately returned by the call. The body makes progress when the future is polled, for example by awaiting it in a running async context; it may suspend while waiting. An executor is responsible for polling a task, and calling an async function alone does not schedule it."),
        span: src.span(keyword),
        type_info: None,
    })
}
