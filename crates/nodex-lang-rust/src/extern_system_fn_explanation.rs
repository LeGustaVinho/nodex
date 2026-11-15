//! Explain the target-dependent `extern "system"` function ABI.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" { return None; }
    let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"system\"" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "System calling convention".into(),
        text: format!("extern \"system\" defines {} using the target platform's system ABI: the calling convention determines how arguments and return values cross the function boundary. Its precise convention depends on the compilation target. For example, on 32-bit x86 Windows non-variadic functions use the system's stdcall convention; on many other targets system follows the C ABI. This is a function definition with a body, not by itself a declaration that loads a native library. ABI compatibility and the safety of calling across a foreign-language boundary must still be checked; the keyword alone does not guarantee either.", src.norm(name)),
        span: src.span(external), type_info: None,
    })
}
