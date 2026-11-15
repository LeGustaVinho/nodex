//! Explain Rust function definitions using the C-unwind ABI.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() || node.has_error() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C-unwind\"" { return None; }
    Some(ContractItem {
        group: "C-unwind calling convention".into(),
        text: "`extern \"C-unwind\"` defines this Rust function with the target's C calling convention and permits unwinding across this ABI boundary. Unwinding is the process of leaving stack frames during a panic or exception; it does not guarantee that every foreign exception can be caught by Rust or that a panic will unwind (the program may be configured to abort). The body remains Rust code. This ABI alone does not export an unmangled symbol, make parameter or return types C-compatible, or make a foreign call safe; an explicit `unsafe` modifier can add caller obligations.".into(),
        span: src.span(external), type_info: None,
    })
}
