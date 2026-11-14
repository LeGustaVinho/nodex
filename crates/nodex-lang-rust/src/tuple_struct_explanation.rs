//! Explain declarations such as `struct Color(u8, u8, u8);`.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" { return None; }
    let body = node.child_by_field_name("body")?;
    if body.kind() != "ordered_field_declaration_list" { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let fields: Vec<_> = named_children(body).into_iter()
        .filter(|field| !matches!(field.kind(), "visibility_modifier" | "attribute_item") && !field.is_extra())
        .map(|field| src.norm(field)).collect();
    let description = if fields.is_empty() {
        "It has no fields.".to_owned()
    } else {
        format!("Its {} positional field{} {}: {}.", fields.len(), if fields.len() == 1 { "" } else { "s" }, if fields.len() == 1 { "has this type" } else { "have these types in order" }, fields.join(", "))
    };
    Some(ContractItem {
        group: "Tuple struct".into(),
        text: format!("`struct` defines {name} as a distinct named type. Parentheses list fields by position instead of giving them field names. {description} Create a value with the type's constructor, supplying values in the same order (when the constructor is accessible); access its fields with .0, .1, and so on, subject to field visibility."),
        span: src.span(node),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn explain_first(text: &str) -> Option<ContractItem> {
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "tuple.rs", index: &index };
        explain(src, tree.root_node().named_child(0).unwrap())
    }
    #[test]
    fn explains_positional_struct_and_not_other_struct_shapes() {
        let color = explain_first("struct Color(u8, u8, u8);").unwrap();
        assert!(color.text.contains("Color as a distinct named type"));
        assert!(color.text.contains("3 positional fields"));
        assert!(color.text.contains("u8, u8, u8"));
        assert!(color.text.contains(".0, .1"));
        assert!(explain_first("struct Point { x: i32 }").is_none());
        assert!(explain_first("struct Marker;").is_none());
    }
}
