//! Explain enumerations made up of variants without associated fields.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{Src, named_children};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "enum_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    let variants: Vec<_> = named_children(body).into_iter()
        .filter(|child| child.kind() == "enum_variant")
        .collect();
    if variants.is_empty() || variants.iter().any(|variant| variant.child_by_field_name("body").is_some()) {
        return None;
    }
    let enum_name = src.norm(name);
    let names: Vec<_> = variants.iter().filter_map(|variant| variant.child_by_field_name("name"))
        .map(|name| src.norm(name)).collect();
    if names.len() != variants.len() { return None; }
    let example = &names[0];
    Some(ContractItem {
        group: "Enumeration".into(),
        text: format!("enum defines {enum_name} as a type with these named alternatives, called variants: {}. A value of {enum_name} is exactly one variant at a time. These variants carry no additional fields; write {enum_name}::{example} to use the first one as a value. The declaration defines the choices; it does not create a value by itself.", names.join(", ")),
        span: src.span(node),
        type_info: None,
    })
}
