//! Connect a type alias to its function pointer signature.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" { return None; }
    let parent = node.parent()?;
    if parent.kind() != "source_file" && !(parent.kind() == "declaration_list" && parent.parent().is_some_and(|p| p.kind() == "mod_item")) { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let ty = node.child_by_field_name("type")?;
    if ty.kind() != "function_type" || ty.child_by_field_name("trait").is_some() { return None; }
    if named_children(ty).into_iter().any(|part| part.kind() == "function_modifiers") { return None; }
    let params = src.norm(ty.child_by_field_name("parameters")?);
    let result = ty.child_by_field_name("return_type").map(|part| src.norm(part)).unwrap_or_else(|| "()".into());
    let keyword = children(ty).into_iter().find(|part| part.kind() == "fn")?;
    Some(ContractItem {
        group: "Function pointer alias".into(),
        text: format!("`{name}` is another name for the function pointer type `{}`; it creates neither a new distinct type nor a function. A value of this type points to a compatible function and can be called with argument types {params}; the call returns {result}. The parentheses list input types and `->` introduces the return type. A compatible function item or non-capturing closure can coerce to this pointer type, but a closure capturing its environment cannot. The alias alone does not select or call any particular function.", src.norm(ty)),
        span: src.span(keyword), type_info: None,
    })
}
