//! Explain explicitly C-ABI function pointer types.
//! Rust Reference: https://doc.rust-lang.org/reference/types/function-pointer.html
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "function_type" || node.child_by_field_name("trait").is_some() || node.has_error() { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let external = named_children(modifiers).into_iter().find(|part| part.kind() == "extern_modifier")?;
    let abi = named_children(external).into_iter().find(|part| part.kind() == "string_literal")?;
    if src.text(abi) != "\"C\"" { return None; }
    let parameters = node.child_by_field_name("parameters")?;
    let result = node.child_by_field_name("return_type").map(|ty| src.norm(ty)).unwrap_or_else(|| "()".into());
    Some((external, format!("`{}` is a function pointer type, not a function call: a value of this type can refer to a compatible function taking {} and returning {}. `extern \"C\"` specifies the target platform's C calling convention, including how inputs and the result are passed. The ABI and signature must be compatible with the function being called; the spelling alone does not establish that its types are suitable for a C interface or that a particular external symbol exists. If the pointer type also says `unsafe`, calling through it requires an unsafe context and the caller must uphold its safety requirements.", src.norm(node), src.norm(parameters), result)))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut stack = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = stack.pop() {
        if node != declaration && matches!(node.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some((site, text)) = explain(src, node) { items.push(ContractItem { group: "C ABI function pointer".into(), text, span: src.span(site), type_info: None }); }
        stack.extend(named_children(node).into_iter().rev());
    }
    items
}

pub(crate) fn local<'t>(src: Src<'_>, declaration: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if declaration.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = declaration.child_by_field_name("type") else { return Vec::new(); };
    let mut stack = vec![ty];
    let mut items = Vec::new();
    while let Some(node) = stack.pop() {
        if let Some((site, text)) = explain(src, node) { items.push((site, ReadingContent::Detail { label: "C ABI function pointer".into(), text, definition: None, borrow: false })); }
        stack.extend(named_children(node).into_iter().rev());
    }
    items
}
