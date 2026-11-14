//! Explain unsafe function pointer types, without guessing the pointed-to function.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, String)> {
    if node.kind() != "function_type" { return None; }
    let modifiers = named_children(node).into_iter().find(|part| part.kind() == "function_modifiers")?;
    let keyword = children(modifiers).into_iter().find(|part| part.kind() == "unsafe")?;
    let parameters = node.child_by_field_name("parameters")?;
    let result = node.child_by_field_name("return_type").map(|ty| src.norm(ty)).unwrap_or_else(|| "()".into());
    Some((keyword, format!("{} is the type of a function pointer: a value that can refer to a function with parameters {} and result {}. `unsafe` means calling through this pointer requires an unsafe context; the caller must uphold the particular function's safety requirements. The signature alone does not reveal those requirements. Declaring or storing the pointer does not call it, and an unsafe function can still contain safe operations.", src.norm(node), src.norm(parameters), result)))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut todo = vec![node];
    let mut out = Vec::new();
    while let Some(part) = todo.pop() {
        if part != node && matches!(part.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some((keyword, text)) = explain(src, part) {
            out.push(ContractItem { group: "Unsafe function pointer".into(), text, span: src.span(keyword), type_info: None });
        }
        todo.extend(named_children(part));
    }
    out
}

pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    let (keyword, text) = explain(src, node)?;
    let declaration = std::iter::successors(node.parent(), |parent| parent.parent())
        .find(|ancestor| matches!(ancestor.kind(), "let_declaration" | "function_item" | "closure_expression" | "const_item" | "static_item"))?;
    if declaration.kind() != "let_declaration" { return None; }
    let ty = declaration.child_by_field_name("type")?;
    if node.start_byte() < ty.start_byte() || node.end_byte() > ty.end_byte() { return None; }
    Some((keyword, ReadingContent::Detail { label: "Unsafe function pointer".into(), text, definition: None, borrow: false }))
}
