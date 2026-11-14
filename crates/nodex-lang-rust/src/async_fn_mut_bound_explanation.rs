//! Explain the `AsyncFnMut(...)` callable trait bound without guessing resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" | "impl_item") { return Vec::new(); }
    let body = declaration.child_by_field_name("body");
    let mut stack = vec![declaration];
    let mut facts = Vec::new();
    while let Some(node) = stack.pop() {
        if Some(node) == body || node.kind() == "token_tree" { continue; }
        if node.kind() == "function_type" {
            if let Some(name) = node.child_by_field_name("trait") {
                let written = src.norm(name);
                let final_segment = written.rsplit("::").next().unwrap_or("");
                if final_segment == "AsyncFnMut" {
                    let inputs = node.child_by_field_name("parameters").map(|p| src.norm(p)).unwrap_or_else(|| "(...)".into());
                    facts.push(ContractItem {
                        group: "Mutable async callable bound".into(),
                        text: format!("{} specifies a callable signature with input types {}. If {} resolves to Rust's standard AsyncFnMut trait, a type satisfying this bound can be called with those inputs through mutable access to the callable. The call produces a future; the asynchronous work advances when that future is polled or awaited. The future may borrow from the callable's captured state, so another mutable call may have to wait until that borrow ends. Calling need not consume the callable, and repeated calls remain subject to borrowing rules. This requirement describes allowed behavior; it does not itself make a call. The name's spelling alone does not prove which trait it resolves to.", src.norm(node), inputs, written),
                        span: src.span(node), type_info: None,
                    });
                }
            }
        }
        stack.extend(named_children(node).into_iter().rev());
    }
    facts
}
