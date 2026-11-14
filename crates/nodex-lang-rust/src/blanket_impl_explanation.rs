//! Explain a trait implemented for a bounded type parameter.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

/// Attach explanations to the containing module, including implementations with no members.
pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "impl_item" || children(item).iter().any(|child| child.kind() == "!") { return None; }
        let implemented_trait = item.child_by_field_name("trait")?;
        let target = item.child_by_field_name("type")?;
        if target.kind() != "type_identifier" { return None; }
        let parameters = item.child_by_field_name("type_parameters")?;
        let parameter = named_children(parameters).into_iter().find(|parameter| {
            parameter.kind() == "type_parameter" && parameter.child_by_field_name("name").is_some_and(|name| src.norm(name) == src.norm(target))
        })?;
        let bounds = parameter.child_by_field_name("bounds")?;
        let bounds_text = src.norm(bounds);
        let bounds_text = bounds_text.trim_start_matches(':').trim();
        if bounds_text.is_empty() { return None; }
        let name = src.norm(target);
        let trait_name = src.norm(implemented_trait);
        Some(ContractItem {
            group: "Generic trait implementation".into(),
            text: format!("impl declares an implementation of the trait {trait_name} for {name}. Here {name} is a type parameter, not a single concrete type: each type substituted for {name} must satisfy {bounds_text}, as well as any other bounds or where-clause requirements. For every type that meets all requirements, this implementation provides {trait_name}; the braces contain its associated definitions and can be empty when none are required. Rust still checks coherence and overlap rules."),
            span: src.header_span(item, item.child_by_field_name("body")),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_bounded_blanket_impl_even_when_empty_and_only_in_its_own_module() {
        let text = "trait Trait {} trait Other {} impl<T: Trait> Other for T {} impl Other for Thing {} mod inner { impl<U: Bound> Other for U {} }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let facts = explain(src, root);
        assert_eq!(facts.len(), 1, "{facts:?}");
        assert!(facts[0].text.contains("type parameter"));
        assert!(facts[0].text.contains("must satisfy Trait"));
        assert!(facts[0].text.contains("can be empty"));
        let inner = named_children(root).into_iter().find(|node| node.kind() == "mod_item").unwrap();
        let facts = explain(src, inner);
        assert_eq!(facts.len(), 1);
        assert!(facts[0].text.contains("must satisfy Bound"));
    }
}
