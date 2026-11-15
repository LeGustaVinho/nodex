//! Distinguish unit, empty tuple and empty named-field enum variants.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let owner = match node.kind() {
        "enum_item" => node,
        "enum_variant" => node.parent()?.parent().filter(|owner| owner.kind() == "enum_item")?,
        _ => return None,
    };
    let enum_name = src.norm(owner.child_by_field_name("name")?);
    let body = owner.child_by_field_name("body")?;
    let variants: Vec<_> = named_children(body).into_iter().filter(|part| part.kind() == "enum_variant").collect();
    let empty = |variant: &Node<'_>, shape: &str| {
        variant.child_by_field_name("body").is_some_and(|body| body.kind() == shape && named_children(body).into_iter().all(|part| part.is_extra()))
    };
    let unit = variants.iter().find(|variant| variant.child_by_field_name("body").is_none())?;
    let tuple = variants.iter().find(|variant| empty(variant, "ordered_field_declaration_list"))?;
    let record = variants.iter().find(|variant| empty(variant, "field_declaration_list"))?;
    let unit = src.norm(unit.child_by_field_name("name")?);
    let tuple = src.norm(tuple.child_by_field_name("name")?);
    let record = src.norm(record.child_by_field_name("name")?);
    Some(ContractItem {
        group: "Three fieldless enum variant forms".into(),
        text: format!("{enum_name} defines distinct alternatives even though these three variants carry no fields. {unit} is a unit variant, written as {enum_name}::{unit} as a value. {tuple} is a tuple-style variant with zero fields, constructed as {enum_name}::{tuple}(). {record} is a named-field variant with zero fields, constructed as {enum_name}::{record} {{}}. The parentheses and braces describe different variant forms rather than optional decoration. Each value holds one variant, so a match can distinguish these cases; the declaration itself does not create a value."),
        span: src.span(node), type_info: None,
    })
}
