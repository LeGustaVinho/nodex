//! Explain a system-ABI definition whose boundary permits unwinding.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" { return None; }
    let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"system-unwind\"" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "System calling convention with unwinding".into(),
        text: format!("`extern \"system-unwind\"` defines {} with the target-dependent system calling convention, determining how arguments and return values cross the function boundary. It uses the same calling convention as `system` but permits unwinding across that boundary: unwinding is the process of leaving calls while cleaning up after a panic or native exception. Whether a Rust panic actually unwinds also depends on the panic runtime: with `panic=abort` it aborts instead. The ABI does not make foreign exceptions or cross-language calls automatically safe or compatible. This is a definition, not by itself a library import.", src.norm(name)),
        span: src.span(external), type_info: None,
    })
}
