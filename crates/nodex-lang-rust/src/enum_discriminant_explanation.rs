//! Beginner-facing explanations of explicitly assigned enum discriminants.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    let variants = match node.kind() {
        "enum_item" => node.child_by_field_name("body")
            .map(named_children).unwrap_or_default(),
        "enum_variant" => vec![node],
        _ => return Vec::new(),
    };
    variants.into_iter().filter_map(|variant| {
        if variant.kind() != "enum_variant" { return None; }
        let name = variant.child_by_field_name("name")?;
        let value = variant.child_by_field_name("value")?;
        Some(ContractItem {
            group: "Enum discriminant".into(),
            text: format!(
                "The `= {}` after variant {} explicitly assigns its discriminant: an integer used to distinguish this variant from the enum's other variants. The expression is evaluated as a constant. This is a tag for the variant, not a field holding extra data; it does not by itself promise a particular memory layout.",
                src.norm(value), src.norm(name),
            ),
            span: src.span(variant),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_each_written_discriminant_without_guessing_implicit_values() {
        let text = "enum E { A = 1, B = 2, C, D = 1 + 3 } struct S;";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "enum.rs", index: &index };
        let nodes = named_children(tree.root_node());
        let explained = explain(src, nodes[0]);
        assert_eq!(explained.len(), 3);
        assert!(explained[0].text.contains("`= 1` after variant A"));
        assert!(explained[1].text.contains("`= 2` after variant B"));
        assert!(explained[2].text.contains("`= 1 + 3` after variant D"));
        assert!(explained.iter().all(|item| item.text.contains("not a field")));
        assert!(explain(src, nodes[1]).is_empty());
        let variants = named_children(nodes[0].child_by_field_name("body").unwrap());
        assert!(explain(src, variants[2]).is_empty());
        assert_eq!(explain(src, variants[0]).len(), 1);
    }
}
