//! Explain a variadic foreign function declaration in an unsafe C ABI block.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_signature_item" || node.has_error() { return None; }
    let block = node.parent().and_then(|body| body.parent()).filter(|part| part.kind() == "foreign_mod_item" && !part.has_error())?;
    if !children(block).iter().any(|part| part.kind() == "unsafe") { return None; }
    let modifier = named_children(block).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(modifier).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let args = named_children(parameters);
    let variadic = args.last().filter(|part| part.kind() == "variadic_parameter" && !part.has_error())?;
    let dots = children(*variadic).into_iter().find(|part| part.kind() == "...").unwrap_or(*variadic);
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Variadic foreign C function".into(),
        text: format!("This declaration describes the foreign function {} using the target's C ABI; it does not provide a Rust body. The final ... means a call may supply zero or more additional arguments after the fixed parameters. Those extra argument types and count are not described by this declaration, so callers must follow the actual foreign function's contract and C ABI, including any required argument promotions or explicit conversions. Passing an unexpected number or type can cause undefined behavior. Foreign functions are unsafe to call by default; an explicitly safe declaration is possible only when its safety requirements are met, including that it does not access variadic arguments. The declaration alone cannot verify the implementation or its contract.", src.norm(name)),
        span: src.span(dots), type_info: None,
    })
}
