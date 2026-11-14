//! Describe a named-field Rust struct declaration to readers new to Rust.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let body = node.child_by_field_name("body")?;
    if body.kind() != "field_declaration_list" { return None; }
    let fields: Vec<_> = named_children(body).into_iter()
        .filter(|field| field.kind() == "field_declaration")
        .filter_map(|field| Some((field.child_by_field_name("name")?, field.child_by_field_name("type")?)))
        .collect();
    let descriptions: Vec<_> = fields.iter().map(|(name, ty)|
        format!("{}: {}", src.norm(*name), src.norm(*ty))).collect();
    let field_text = if descriptions.is_empty() {
        "It has no fields.".to_owned()
    } else {
        format!("Each value has these named fields: {}. The type after each colon specifies the kind of value stored in that field.", descriptions.join(", "))
    };
    let float_text = if fields.iter().any(|(_, ty)| src.norm(*ty) == "f32") {
        " f32 is a 32-bit floating-point number."
    } else { "" };
    Some(ContractItem {
        group: "Structure".into(),
        text: format!("struct defines {} as a new type that groups related data. {}{} This declaration describes the shape of values; it does not create a value by itself.", src.norm(name), field_text, float_text),
        span: src.span(node),
        type_info: None,
    })
}
