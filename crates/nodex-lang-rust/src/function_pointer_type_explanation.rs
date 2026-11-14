//! Explain the written type of a safe Rust function pointer.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "function_type" || node.child_by_field_name("trait").is_some() { return None; }
    // Unsafe function pointers need a separate explanation of the call obligation.
    if named_children(node).iter().any(|child| child.kind() == "function_modifiers" && children(*child).iter().any(|part| part.kind() == "unsafe")) { return None; }
    let params = node.child_by_field_name("parameters")?;
    let result = node.child_by_field_name("return_type").map(|ty| src.norm(ty)).unwrap_or_else(|| "()".into());
    Some(format!("{} is a function pointer type: a value of this type identifies a function that can be called with the parameter types {} and returns {}. The types inside the parentheses describe inputs, and -> introduces the result type. This is a type, not a function call or definition. A compatible function can be assigned to a variable of this type; a closure that captures local variables is not a function pointer. Calling the value executes the function.", src.norm(node), src.norm(params), result))
}

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if !matches!(declaration.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut pending = vec![declaration];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if node != declaration && matches!(node.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some(text) = describe(src, node) { items.push(ContractItem { group: "Function pointer type".into(), text, span: src.span(node), type_info: None }); }
        pending.extend(named_children(node));
    }
    items
}

pub(crate) fn local<'a>(src: Src<'_>, declaration: Node<'a>) -> Vec<(Node<'a>, ReadingContent)> {
    if declaration.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = declaration.child_by_field_name("type") else { return Vec::new(); };
    let mut pending = vec![ty];
    let mut items = Vec::new();
    while let Some(node) = pending.pop() {
        if let Some(text) = describe(src, node) {
            let highlight = children(node).into_iter().find(|part| part.kind() == "fn").unwrap_or(node);
            items.push((highlight, ReadingContent::Detail { label: "Function pointer type".into(), text, definition: None, borrow: false }));
        }
        pending.extend(named_children(node));
    }
    items
}
