//! Explain mixed lifetime, type, and literal constant arguments of a type.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "generic_type" || node.has_error() { return None; }
    let head = node.child_by_field_name("type")?;
    let arguments = node.child_by_field_name("type_arguments")?;
    let parts = named_children(arguments);
    let [lifetime, ty, constant] = parts.as_slice() else { return None; };
    if lifetime.kind() != "lifetime" || !matches!(ty.kind(), "type_identifier" | "scoped_type_identifier" | "primitive_type") || constant.kind() != "integer_literal" { return None; }
    Some(format!("`{}` names a type with three generic arguments: `{}` is a lifetime argument describing a validity relationship, `{}` is a type argument, and `{}` is a constant integer argument. The declaration of `{}` determines what these parameters mean, their constraints, and whether these arguments are valid. The lifetime does not by itself extend any value's lifetime, and the constant's role is defined by the type rather than necessarily being a length. This specifies a type; it does not construct a value. The written name alone does not establish which declaration it resolves to.", src.norm(node), src.norm(*lifetime), src.norm(*ty), src.norm(*constant), src.norm(head)))
}

fn occurrences<'t>(src: Src<'_>, root: Node<'t>, skip_body: bool) -> Vec<(Node<'t>, String)> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body { continue; }
        if let Some(text) = describe(src, part) { result.push((part, text)); }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(src, node, true).into_iter().map(|(site, text)| ContractItem { group: "Mixed generic type arguments".into(), text, span: src.span(site), type_info: None }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(src, ty, false).into_iter().map(|(site, text)| (site, ReadingContent::Detail { label: "Mixed generic type arguments".into(), text, definition: None, borrow: false })).collect()
}
