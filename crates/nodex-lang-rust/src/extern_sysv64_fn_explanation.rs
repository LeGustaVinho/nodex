//! Explain the System V x86-64 calling convention on functions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let (external, definition) = match node.kind() {
        "function_item" => {
            let modifiers = children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
            (named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?, true)
        }
        "function_signature_item" => {
            let block = node.parent()?.parent()?;
            if block.kind() != "foreign_mod_item" { return None; }
            (named_children(block).into_iter().find(|part| part.kind() == "extern_modifier")?, false)
        }
        _ => return None,
    };
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"sysv64\"" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "System V calling convention".into(),
        text: format!("`extern \"sysv64\"` selects the System V x86-64 calling convention for {}. This ABI defines low-level details such as how arguments and results pass through registers or the stack. It is available only on x86-64 compilation targets, and matches the C ABI on non-Windows x86-64 targets; the source alone does not establish the target or whether this build supports it. {} The ABI alone does not ensure compatible parameter and result types or layouts, export an unmangled symbol, or establish that calls across a foreign-language boundary are safe.", src.norm(name), if definition { "The braces contain a Rust function body; this does not by itself import a library." } else { "This foreign block declares the function; linked code must provide its implementation." }),
        span: src.span(external), type_info: None,
    })
}
