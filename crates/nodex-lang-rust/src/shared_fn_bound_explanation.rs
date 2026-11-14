//! Explain the callable bound `Fn(Arguments) -> Output` without guessing name resolution.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "trait_item" | "struct_item" | "enum_item" | "impl_item" | "type_item") { return Vec::new(); }
    let skip = node.child_by_field_name("body");
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == skip || part.kind() == "token_tree" { continue; }
        if part.kind() == "function_type" {
            if let (Some(trait_name), Some(parameters)) = (part.child_by_field_name("trait"), part.child_by_field_name("parameters")) {
                let name = src.norm(trait_name);
                if matches!(name.as_str(), "Fn" | "std::ops::Fn" | "core::ops::Fn" | "::std::ops::Fn" | "::core::ops::Fn") {
                    let output = part.child_by_field_name("return_type").map(|n| src.norm(n)).unwrap_or_else(|| "()".into());
                    result.push(ContractItem {
                        group: "Shared callable bound".into(),
                        text: format!("{} is a requirement on a callable type: it accepts arguments described by {} and returns {}. If {} resolves to Rust's standard Fn trait, a call uses shared access to the callable instead of consuming it or requiring exclusive mutable access. Such a callable can be called repeatedly when each call completes; captured data may still change through interior mutability. The callable may be a closure or another implementing value. The written name alone does not establish its identity; the compiler checks that the concrete type meets this bound.", src.norm(part), src.norm(parameters), output, name),
                        span: src.span(part), type_info: None,
                    });
                }
            }
            continue;
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
