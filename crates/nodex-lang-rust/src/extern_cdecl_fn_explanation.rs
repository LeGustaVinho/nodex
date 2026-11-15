//! Explain a Rust function definition using the x86 `cdecl` calling convention.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return None; }
    let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"cdecl\"" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Cdecl calling convention".into(),
        text: format!("`extern \"cdecl\"` defines {} using the cdecl calling convention, which specifies how callers and the function pass arguments and return values across the boundary. Rust documents this ABI for 32-bit x86 targets, where it corresponds to the C cdecl convention; availability depends on the compilation target, which this syntax alone does not establish. The body here is Rust code, not by itself a native library import. Both sides must still agree on the ABI and compatible types; this qualifier alone does not guarantee FFI safety or an unmangled exported name.", src.norm(name)),
        span: src.span(external), type_info: None,
    })
}
