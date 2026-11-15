//! Explain a named ABI in a foreign block without assuming it is supported.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_signature_item" || node.has_error() { return None; }
    let block = node.parent()?.parent()?;
    if block.kind() != "foreign_mod_item" || block.has_error() { return None; }
    let modifier = named_children(block).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(modifier).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"custom\"" { return None; }
    Some(ContractItem {
        group: "Named foreign ABI".into(),
        text: "The string in `extern \"custom\" { ... }` names the intended ABI: the rules for passing arguments, returning values, and crossing the boundary to external code. The block declares its functions; it does not define or call them. The compiler must recognize and support the ABI name for the target and configuration; an arbitrary name such as `custom` is not automatically a valid or user-defined calling convention. If unsupported, the declaration is rejected. Even when a named ABI is supported, the declarations must match the actual external definitions, and the ABI name alone does not establish their safety or guarantee linking.".into(),
        span: src.span(abi), type_info: None,
    })
}
