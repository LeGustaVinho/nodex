//! Explain AsyncFn bounds without assuming the trait name resolves to std.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Find callable trait bounds in the declaration's own header, not its body.
pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "trait_item" | "impl_item" | "type_item") { return Vec::new(); }
    let body = node.child_by_field_name("body");
    let mut pending = vec![node];
    let mut output = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body || part.kind() == "token_tree" { continue; }
        if part.kind() == "function_type" {
            if let Some(trait_name) = part.child_by_field_name("trait") {
                let written = src.norm(trait_name);
                if matches!(written.as_str(), "AsyncFn" | "std::ops::AsyncFn" | "core::ops::AsyncFn") {
                    let target = std::iter::successors(part.parent(), |parent| parent.parent())
                        .take_while(|parent| *parent != node)
                        .find_map(|parent| match parent.kind() {
                            "type_parameter" => parent.child_by_field_name("name"),
                            "where_predicate" => parent.child_by_field_name("left"),
                            _ => None,
                        });
                    if let Some(target) = target {
                        output.push(ContractItem {
                            group: "Async callable bound".into(),
                            text: format!("The bound {} requires {} to satisfy the named callable trait for the argument types in parentheses. If {} resolves to Rust's standard AsyncFn trait, the callable can be invoked with those arguments and each invocation returns a future; the asynchronous operation makes progress when that future is polled or awaited, not merely by writing this bound. AsyncFn permits calls through shared access, and the future may borrow from the callable. Any written -> result type describes the future's output. The spelling alone does not prove which trait the name resolves to.", src.norm(part), src.norm(target), written),
                            span: src.span(part), type_info: None,
                        });
                    }
                }
            }
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    output
}
