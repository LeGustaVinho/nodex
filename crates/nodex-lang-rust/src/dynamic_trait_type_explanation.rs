//! Explain the `dyn Trait` type independently of the pointer that contains it.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if node.kind() != "dynamic_type" { return None; }
    let written = src.norm(node);
    Some(format!("{written} is a trait object type. `dyn` hides the concrete implementing type and lets code use the trait's supported methods through runtime dispatch: the method implementation is selected for the concrete value at runtime. The trait must be dyn-compatible. A trait object is dynamically sized, so it is normally used behind a pointer such as a reference or Box; the pointer's form determines whether the value is borrowed or owned. The written trait name still needs normal name resolution."))
}

fn occurrences<'t>(src: Src<'_>, root: Node<'t>, skip_body: bool) -> Vec<(Node<'t>, String)> {
    let body = if skip_body { root.child_by_field_name("body") } else { None };
    let mut pending = vec![root];
    let mut found = Vec::new();
    while let Some(node) = pending.pop() {
        if Some(node) == body || node.kind() == "token_tree" { continue; }
        if let Some(text) = describe(src, node) { found.push((node, text)); }
        pending.extend(named_children(node).into_iter().rev());
    }
    found
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item" | "field_declaration" | "type_item" | "const_item" | "static_item") { return Vec::new(); }
    occurrences(src, node, true).into_iter().map(|(ty, text)| ContractItem { group: "Dynamic trait object".into(), text, span: src.span(ty), type_info: None }).collect()
}

pub(crate) fn local<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, ReadingContent)> {
    if node.kind() != "let_declaration" { return Vec::new(); }
    let Some(ty) = node.child_by_field_name("type") else { return Vec::new(); };
    occurrences(src, ty, false).into_iter().map(|(site, text)| {
        let highlight = children(site).into_iter().find(|child| child.kind() == "dyn").unwrap_or(site);
        (highlight, ReadingContent::Detail { label: "Dynamic trait object".into(), text, definition: None, borrow: false })
    }).collect()
}
