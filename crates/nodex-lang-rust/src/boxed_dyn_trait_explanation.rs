//! Explain a boxed trait object without assuming the spelling `Box` resolves to std.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn matches_box_dyn<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    if node.kind() != "generic_type" { return None; }
    let head = node.child_by_field_name("type")?;
    if !matches!(src.norm(head).as_str(), "Box" | "std::boxed::Box" | "alloc::boxed::Box") { return None; }
    let args = node.child_by_field_name("type_arguments")?;
    let argument = named_children(args).into_iter().next()?;
    if argument.kind() == "dynamic_type" { return Some((head, argument)); }
    if argument.kind() == "bounded_type" {
        return named_children(argument).into_iter().find(|part| part.kind() == "dynamic_type").map(|dynamic| (head, dynamic));
    }
    None
}

fn explanation(src: Src<'_>, head: Node<'_>, dynamic: Node<'_>) -> String {
    format!("{} names a trait object: the concrete value's type is hidden behind the listed trait and method calls through that trait use runtime dispatch. If {} resolves to Rust's standard Box, the Box owns the dynamically sized value in heap storage; moving the Box transfers that owning handle, and dropping it releases the owned value. The written name alone does not prove which Box type is in scope. Any lifetime or additional trait bounds still constrain the stored value.", src.norm(dynamic), src.norm(head))
}

fn occurrences<'t>(src: Src<'_>, root: Node<'t>, skip_body: bool) -> Vec<(Node<'t>, Node<'t>, Node<'t>)> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body { continue; }
        if let Some((head, dynamic)) = matches_box_dyn(src, node) { found.push((node, head, dynamic)); }
        pending.extend(named_children(node));
    }
    found.sort_by_key(|(node, _, _)| node.start_byte());
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(src, node, true).into_iter().map(|(whole, head, dynamic)| ContractItem {
        group: "Boxed trait object".into(), text: explanation(src, head, dynamic), span: src.span(whole), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(src, ty, false).into_iter().map(|(_, head, dynamic)| (head, ReadingContent::Detail {
        label: "Boxed trait object".into(), text: explanation(src, head, dynamic), definition: None, borrow: false,
    })).collect()
}
