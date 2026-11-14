//! Explain higher-ranked lifetimes in function pointer types.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn explanation<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "function_type" { return None; }
    let binder = named_children(node).into_iter().find(|child| child.kind() == "for_lifetimes")?;
    Some((binder, format!("In {}, {} declares lifetime names local to this function pointer type. The pointer must be usable for every suitable choice of those lifetimes: for example, it can accept a reference with whichever lifetime a particular call supplies, rather than requiring one lifetime fixed when the pointer was stored. Relationships explicitly written between inputs and outputs still apply. This is a requirement on the pointer's signature, not an instruction to call the function or extend the life of any value.", src.norm(node), src.norm(binder))))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "associated_type" | "const_item" | "static_item") { return Vec::new(); }
    let body = node.child_by_field_name("body");
    let value = node.child_by_field_name("value");
    let mut pending = vec![node];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if Some(part) == body || Some(part) == value { continue; }
        if let Some((binder, text)) = explanation(src, part) {
            result.push(ContractItem { group: "Function pointer lifetime binder".into(), text, span: src.span(binder), type_info: None });
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new() };
    let mut pending = vec![ty];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if let Some((binder, text)) = explanation(src, part) {
            result.push((binder, ReadingContent::Detail { label: "Function pointer lifetime binder".into(), text, definition: None, borrow: false }));
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    result
}
