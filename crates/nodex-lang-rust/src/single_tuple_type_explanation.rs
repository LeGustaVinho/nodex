//! Explain the comma that makes a one-element tuple type.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn explain<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, String)> {
    if node.kind() != "tuple_type" { return None; }
    let elements: Vec<_> = named_children(node).into_iter().filter(|part| !part.is_extra()).collect();
    if elements.len() != 1 { return None; }
    let comma = children(node).into_iter().find(|part| part.kind() == ",")?;
    Some((comma, format!("{} is a tuple type with exactly one positional element, whose type is {}. The trailing comma is required: without it, parentheses only group the type and do not create a tuple. A matching value also uses a comma, for example (value,); its element can be accessed as .0.", src.norm(node), src.norm(elements[0]))))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "struct_item" | "union_item" | "enum_variant" | "type_item" | "const_item" | "static_item" | "field_declaration") { return Vec::new(); }
    let mut todo = vec![node];
    let mut out = Vec::new();
    while let Some(part) = todo.pop() {
        if part != node && matches!(part.kind(), "block" | "declaration_list" | "function_item" | "function_signature_item" | "impl_item" | "trait_item" | "closure_expression" | "token_tree") { continue; }
        if let Some((comma, text)) = explain(src, part) {
            out.push(ContractItem { group: "One-element tuple type".into(), text, span: src.span(comma), type_info: None });
        }
        todo.extend(named_children(part));
    }
    out
}

pub(crate) fn local<'a>(src: Src<'_>, node: Node<'a>) -> Option<(Node<'a>, ReadingContent)> {
    let (comma, text) = explain(src, node)?;
    let declaration = std::iter::successors(node.parent(), |parent| parent.parent())
        .find(|ancestor| matches!(ancestor.kind(), "let_declaration" | "function_item" | "closure_expression" | "const_item" | "static_item"))?;
    if declaration.kind() != "let_declaration" { return None; }
    let ty = declaration.child_by_field_name("type")?;
    if node.start_byte() < ty.start_byte() || node.end_byte() > ty.end_byte() { return None; }
    Some((comma, ReadingContent::Detail { label: "One-element tuple type".into(), text, definition: None, borrow: false }))
}
