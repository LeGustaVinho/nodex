//! Explain bounds on a type declared by a trait.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, node: Node<'_>, trait_name: &str) -> Option<ContractItem> {
    if node.kind() != "associated_type" || node.has_error() { return None; }
    let name = node.child_by_field_name("name")?;
    let bounds = named_children(node).into_iter().find(|n| n.kind() == "trait_bounds")?;
    let written = src.norm(bounds);
    let requirements = written.trim_start_matches(':').trim();
    if requirements.is_empty() { return None; }
    let clone_note = if requirements.split(|c: char| !c.is_alphanumeric() && c != '_').any(|part| part == "Clone") {
        " If Clone resolves to Rust's standard Clone trait, the chosen type supports explicit cloning via clone()."
    } else { "" };
    Some(ContractItem {
        group: "Associated type declaration bound".into(),
        text: format!("In trait {trait_name}, type {} declares an associated type chosen by each implementation. The colon requires the chosen type to satisfy {requirements}; it does not assign a concrete type.{clone_note} Name resolution determines which trait each bound denotes, and the declaration alone does not prove an implementation satisfies it.", src.norm(name)),
        span: src.span(node), type_info: None,
    })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "trait_item" {
        let Some(name) = node.child_by_field_name("name") else { return Vec::new() };
        let Some(body) = node.child_by_field_name("body") else { return Vec::new() };
        return named_children(body).into_iter().filter_map(|n| describe(src, n, &src.norm(name))).collect();
    }
    if node.kind() == "associated_type" {
        let Some(owner) = node.parent().and_then(|p| p.parent()).filter(|p| p.kind() == "trait_item") else { return Vec::new() };
        let Some(name) = owner.child_by_field_name("name") else { return Vec::new() };
        return describe(src, node, &src.norm(name)).into_iter().collect();
    }
    Vec::new()
}
