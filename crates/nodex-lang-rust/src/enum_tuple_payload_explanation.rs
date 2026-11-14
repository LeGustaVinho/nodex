//! Explain enums whose alternatives include positional data.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe the enum declaration without assuming knowledge of tagged unions.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "enum_item" { return None; }
    let name = src.norm(node.child_by_field_name("name")?);
    let body = node.child_by_field_name("body")?;
    let variants: Vec<_> = named_children(body).into_iter()
        .filter(|v| v.kind() == "enum_variant").collect();
    if !variants.iter().any(|v| v.child_by_field_name("body")
        .is_some_and(|body| body.kind() == "ordered_field_declaration_list")) { return None; }
    let descriptions: Vec<_> = variants.iter().filter_map(|variant| {
        let variant_name = src.norm(variant.child_by_field_name("name")?);
        match variant.child_by_field_name("body") {
            None => Some(format!("{variant_name} carries no fields")),
            Some(fields) if fields.kind() == "ordered_field_declaration_list" => {
                let types: Vec<_> = named_children(fields).into_iter()
                    .filter(|n| !n.is_extra() && !matches!(n.kind(), "attribute_item" | "visibility_modifier"))
                    .map(|n| src.norm(n)).collect();
                Some(format!("{variant_name} carries positional fields of types {}", types.join(", ")))
            }
            Some(_) => Some(format!("{variant_name} carries named fields")),
        }
    }).collect();
    Some(ContractItem {
        group: "Enum alternatives".into(),
        text: format!("`enum` defines {name} as a type with alternative forms called variants. Each value is exactly one variant and contains the fields belonging to that variant. {}. The types in parentheses describe values supplied in that order; for example, a tuple-style variant is constructed as {name}::Variant(values). Code can use `match` to choose behavior and extract data for each variant. This declaration defines the type; it does not create a value.", descriptions.join("; ")),
        span: src.span(node),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn inspect(text: &str) -> Option<ContractItem> {
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "enum.rs", index: &index };
        explain(src, tree.root_node().named_child(0).unwrap())
    }
    #[test]
    fn explains_unit_and_positional_variants_without_claiming_a_value_was_created() {
        let item = inspect("enum Message { Quit, Move(i32, i32) }").unwrap();
        assert!(item.text.contains("exactly one variant"));
        assert!(item.text.contains("Quit carries no fields"));
        assert!(item.text.contains("Move carries positional fields of types i32, i32"));
        assert!(item.text.contains("does not create a value"));
        assert!(inspect("enum State { Idle, Running }").is_none());
        assert!(inspect("struct Pair(i32, i32);").is_none());
    }
}
