//! Explain a Rust function definition explicitly using the C ABI.
//! Rust Reference: https://doc.rust-lang.org/reference/items/external-blocks.html#abi
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_item" || node.child_by_field_name("body").is_none() || node.has_error() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    Some(ContractItem {
        group: "C calling convention".into(),
        text: "`extern \"C\"` gives this Rust function the target platform's C application binary interface (ABI). That convention specifies low-level calling details such as how arguments and return values are passed; the braces still define a Rust function body. Choosing this ABI alone does not export an unmangled symbol, establish that its parameter and return types are C-compatible, or make arbitrary cross-language calls safe. An explicit `unsafe` modifier can impose additional caller obligations. Other ABIs, including `C-unwind`, have their own rules.".into(),
        span: src.span(external), type_info: None,
    })
}
