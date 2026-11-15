//! Explain a function explicitly declared with Rust's native ABI.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return Vec::new(); }
    let Some(modifiers) = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers") else { return Vec::new() };
    let Some(extern_modifier) = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier") else { return Vec::new() };
    let Some(abi) = named_children(extern_modifier).into_iter().find(|part| part.kind() == "string_literal") else { return Vec::new() };
    if src.text(abi) != "\"Rust\"" { return Vec::new(); }
    let name = node.child_by_field_name("name").map(|part| src.norm(part)).unwrap_or_else(|| "this function".into());
    vec![ContractItem {
        group: "Explicit Rust ABI".into(),
        text: format!("`extern \"Rust\"` explicitly chooses Rust's native application binary interface for {name}. The ABI specifies low-level calling conventions, such as how arguments and results are passed; ordinary Rust functions already use the Rust ABI by default. The Rust ABI has no stability guarantees, so this spelling does not establish a stable C-compatible interface across toolchains or targets. It does not by itself export a symbol or make the function unsafe to call; any separate `unsafe` modifier and the actual signature still matter. The compiler and linked code must agree on the ABI and types."),
        span: src.span(extern_modifier), type_info: None,
    }]
}
