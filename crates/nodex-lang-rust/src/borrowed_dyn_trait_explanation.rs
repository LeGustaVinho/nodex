//! Explain a shared borrowed trait object such as `&dyn Drawable`.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn is_borrowed_dyn(node: Node<'_>) -> bool {
    node.kind() == "reference_type"
        && !children(node).iter().any(|child| child.kind() == "mutable_specifier")
        && node.child_by_field_name("type").is_some_and(|ty| ty.kind() == "dynamic_type")
}

fn description(src: Src<'_>, node: Node<'_>) -> String {
    let object = node.child_by_field_name("type").map(|ty| src.norm(ty)).unwrap_or_else(|| "dyn Trait".into());
    format!("&{object} is a shared reference to a trait object. It borrows an existing value rather than taking ownership; the reference must not outlive that value. The concrete type behind the reference is hidden at this point, but it must implement the named trait and satisfy any other bounds. Calls to dispatchable trait methods through this reference select the concrete implementation at runtime. A shared reference normally restricts mutation, although interior-mutability types can permit controlled changes.")
}

fn occurrences<'t>(root: Node<'t>, body: Option<Node<'t>>) -> Vec<Node<'t>> {
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body || node.kind() == "token_tree" { continue; }
        if is_borrowed_dyn(node) { found.push(node); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item" | "associated_type") { return Vec::new(); }
    occurrences(node, node.child_by_field_name("body")).into_iter().map(|reference| ContractItem {
        group: "Borrowed trait object".into(), text: description(src, reference), span: src.span(reference), type_info: None,
    }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(ty, None).into_iter().map(|reference| {
        let highlight = children(reference).into_iter().find(|child| child.kind() == "&").unwrap_or(reference);
        (highlight, ReadingContent::Detail { label: "Borrowed trait object".into(), text: description(src, reference), definition: None, borrow: false })
    }).collect()
}
