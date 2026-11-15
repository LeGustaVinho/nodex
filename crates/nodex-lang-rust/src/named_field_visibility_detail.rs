//! Explain per-field visibility for named struct fields.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

fn describe(src: Src<'_>, field: Node<'_>) -> Option<ContractItem> {
    if field.kind() != "field_declaration" || field.has_error() { return None; }
    let owner = field.parent()?.parent()?;
    if owner.kind() != "struct_item" { return None; }
    let name = field.child_by_field_name("name")?;
    let visibility = children(field).into_iter().find(|c| c.kind() == "visibility_modifier");
    let (span, explanation) = match visibility {
        Some(v) if src.norm(v).chars().filter(|c| !c.is_whitespace()).collect::<String>() == "pub(crate)" => (src.span(v), "`pub(crate)` makes this field accessible within the current crate (compilation unit), subject to access to the struct and its containing path. It does not expose the field to other crates."),
        None => (src.span(name), "With no visibility modifier, this field is private to its declaring module and descendants. Code must also be able to access the struct and its containing path."),
        _ => return None,
    };
    Some(ContractItem { group: "Named field visibility".into(), text: format!("Field {}: {explanation} Each struct field has its own visibility; the struct's own visibility does not automatically make its fields public.", src.norm(name)), span, type_info: None })
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() == "field_declaration" { return describe(src, node).into_iter().collect(); }
    if node.kind() != "struct_item" { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    if body.kind() != "field_declaration_list" { return Vec::new(); }
    named_children(body).into_iter().filter_map(|field| describe(src, field)).collect()
}
