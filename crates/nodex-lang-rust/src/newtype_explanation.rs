//! Explain a one-field tuple struct (the common newtype idiom).
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "struct_item" { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let body = node.child_by_field_name("body")?;
    if body.kind() != "ordered_field_declaration_list" { return None; }
    let fields: Vec<_> = named_children(body).into_iter()
        .filter(|field| !field.is_extra() && !matches!(field.kind(), "visibility_modifier" | "attribute_item"))
        .collect();
    let [field] = fields.as_slice() else { return None; };
    let inner = src.norm(*field);
    Some(ContractItem {
        group: "Single-field type".into(),
        text: format!("{name} is a tuple struct containing one unnamed field of type {inner}; this wrapper is often called a newtype. {name} is a distinct type from {inner}, so one cannot be used where the other is expected without explicitly wrapping or extracting it. Construct a value with {name}(value), and access its field as value.0 where its visibility allows."),
        span: src.span(node),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_a_distinct_single_field_type() {
        let text = "struct UserId(u64); struct Point(i32, i32); struct Named { id: u64 }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "types.rs", index: &index };
        let items = named_children(tree.root_node());
        let item = explain(src, items[0]).unwrap();
        assert!(item.text.contains("distinct type from u64"));
        assert!(item.text.contains("UserId(value)"));
        assert!(item.text.contains("value.0"));
        assert!(explain(src, items[1]).is_none());
        assert!(explain(src, items[2]).is_none());
    }
}
