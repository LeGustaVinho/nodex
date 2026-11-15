//! Explain the optional name in a foreign C variadic declaration.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "function_signature_item" || node.has_error() { return None; }
    let block = node.parent()?.parent()?;
    if block.kind() != "foreign_mod_item" || block.has_error() || !children(block).iter().any(|part| part.kind() == "unsafe") { return None; }
    let external = named_children(block).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let args = named_children(parameters);
    let variadic = args.last().filter(|part| part.kind() == "variadic_parameter" && !part.has_error())?;
    let name = variadic.child_by_field_name("pattern")?;
    if name.kind() != "identifier" { return None; }
    Some(ContractItem {
        group: "Named foreign C variadic parameter".into(),
        text: format!("`{}: ...` names the optional variadic parameter in this foreign function declaration. The name is optional and documents the extra arguments; it is not a fixed argument, a collection passed as one value, or a Rust function body that reads the arguments. Calls may supply zero or more extra arguments after the fixed parameters, subject to the actual foreign function's contract and C ABI, including argument promotions. These types and count are not specified by the declaration. The caller must uphold the foreign function's safety requirements; wrong argument types or counts can cause undefined behavior. The declaration alone does not verify the foreign implementation.", src.norm(name)),
        span: src.span(name), type_info: None,
    })
}
