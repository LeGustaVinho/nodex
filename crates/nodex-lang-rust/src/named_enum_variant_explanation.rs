//! Beginner-facing descriptions of enum variants with named fields.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe named-field variants both on their enum and on the variant symbol.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let variants: Vec<_> = match node.kind() {
        "enum_item" => node.child_by_field_name("body")
            .map(named_children).unwrap_or_default().into_iter()
            .filter(|child| child.kind() == "enum_variant").collect(),
        "enum_variant" => vec![node],
        _ => return Vec::new(),
    };
    variants.into_iter().filter_map(|variant| {
        let body = variant.child_by_field_name("body")?;
        if body.kind() != "field_declaration_list" { return None; }
        let name = src.norm(variant.child_by_field_name("name")?);
        let enum_name = variant.parent().and_then(|node| node.parent())
            .filter(|parent| parent.kind() == "enum_item")
            .and_then(|parent| parent.child_by_field_name("name"))
            .map(|name| src.norm(name));
        let fields: Vec<_> = named_children(body).into_iter()
            .filter(|field| field.kind() == "field_declaration")
            .filter_map(|field| Some((src.norm(field.child_by_field_name("name")?), src.norm(field.child_by_field_name("type")?))))
            .collect();
        let field_description = if fields.is_empty() {
            "The braces contain no fields.".to_owned()
        } else {
            format!("Its named fields are {}. A colon separates each field name from the type of value it holds.", fields.iter().map(|(name, ty)| format!("{name}: {ty}")).collect::<Vec<_>>().join(", "))
        };
        let owner = enum_name.map_or_else(|| "this enum".to_owned(), |name| format!("the {name} enum"));
        Some(ContractItem {
            group: "Named-field enum variant".into(),
            text: format!("{name} is one possible variant of {owner}. A value of the enum holds one variant at a time; when it is {name}, it carries the data described inside these braces. {field_description} This declaration describes the variant; it does not create a value."),
            span: src.span(variant),
            type_info: None,
        })
    }).collect()
}
