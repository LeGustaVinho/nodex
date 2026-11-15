//! Explain a function definition with the explicit `stdcall` ABI.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.has_error() { return None; }
    let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"stdcall\"" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Stdcall calling convention".into(),
        text: format!("`extern \"stdcall\"` defines {} using the stdcall ABI: the calling convention specifies how arguments and return values cross the function boundary. Rust supports this ABI only for 32-bit x86 targets; it is commonly used by the Win32 API on that architecture. The written ABI does not prove the current compilation target supports it, nor that a caller's declaration and argument types match. This function has a Rust body; `extern` here does not by itself load a library or export an unmangled symbol. FFI safety, symbol linkage, and compatible declarations still require separate attention.", src.norm(name)),
        span: src.span(external), type_info: None,
    })
}
