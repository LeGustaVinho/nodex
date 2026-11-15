//! Explain an extern function definition with an omitted ABI string.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.has_error() || node.child_by_field_name("body").is_none() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    if named_children(external).into_iter().any(|part| matches!(part.kind(), "string_literal" | "raw_string_literal")) { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Default extern calling convention".into(),
        text: format!("`extern fn {}` defines a Rust function whose ABI (low-level calling convention) defaults to the target's `\"C\"` ABI when no ABI string is written; `extern \"C\" fn` states the same choice explicitly and is preferred. The ABI controls details such as how arguments and return values are passed. This differs from an ordinary function without `extern`, which uses the Rust ABI. The braces are the function's Rust body, not a foreign declaration. `extern` alone neither makes the function unsafe to call nor exports an unmangled symbol or guarantees C-compatible types; any separate `unsafe` qualifier and the actual signature still matter.", src.norm(name)),
        span: src.span(external), type_info: None,
    })
}
