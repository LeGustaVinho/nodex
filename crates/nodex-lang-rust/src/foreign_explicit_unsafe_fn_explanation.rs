//! Explain an explicitly unsafe foreign function inside `unsafe extern "C"`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_signature_item" || node.has_error() { return Vec::new(); }
    let Some(block) = node.parent().and_then(|body| body.parent()).filter(|parent| parent.kind() == "foreign_mod_item" && !parent.has_error()) else { return Vec::new(); };
    if !children(block).iter().any(|part| part.kind() == "unsafe") { return Vec::new(); }
    let Some(extern_modifier) = named_children(block).into_iter().find(|part| part.kind() == "extern_modifier") else { return Vec::new(); };
    if !named_children(extern_modifier).iter().any(|part| part.kind() == "string_literal" && src.text(*part) == "\"C\"") { return Vec::new(); }
    let Some(modifiers) = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers") else { return Vec::new(); };
    let Some(keyword) = children(modifiers).into_iter().find(|part| part.kind() == "unsafe") else { return Vec::new(); };
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    vec![ContractItem {
        group: "Explicitly unsafe foreign function".into(),
        text: format!("`unsafe extern \"C\"` declares foreign items using the C calling convention; the block does not provide the body of {}. The author of the extern block is responsible for declaring the foreign signature and ABI correctly. The explicit `unsafe fn` marks calls to {} as requiring an unsafe context: the caller must uphold the function's safety preconditions, such as any requirements on pointers and values. The declaration does not itself verify those preconditions, locate a matching linked symbol, or prove the implementation obeys the signature. `unsafe` on the block and `unsafe` on this function describe related but distinct responsibilities.", src.norm(name), src.norm(name)),
        span: src.span(keyword), type_info: None,
    }]
}
