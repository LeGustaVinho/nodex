//! Explain an explicitly written unit return type on a function.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") || node.has_error() { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("return_type").filter(|n| n.kind() == "unit_type") else { return Vec::new(); };
    let is_async = children(node).into_iter().filter(|n| n.kind() == "function_modifiers").any(|n| children(n).iter().any(|c| c.kind() == "async"));
    let completion = if is_async { "Calling this async function produces a future; when that future completes normally, its output is unit." } else { "When the function completes normally, it returns the unit value." };
    vec![ContractItem {
        group: "Explicit unit return".into(),
        text: format!("`-> ()` explicitly states the function's result type is the unit type `()`, which has one value, also written `()`. Omitting the return annotation has the same unit result type. {completion} An empty body or a body ending without a final value expression normally yields unit. Unlike the never type `!`, unit permits normal completion; it is an actual value, even though it carries no meaningful data. A body may still return early, panic, or never finish."),
        span: src.span(ty), type_info: None,
    }]
}
