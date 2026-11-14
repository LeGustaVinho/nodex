//! Explanation of visibility restricted to an ancestor module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, item: Node<'_>) -> Option<ContractItem> {
    let visibility = named_children(item).into_iter().find(|n| n.kind() == "visibility_modifier")?;
    if !children(visibility).iter().any(|n| n.kind() == "in") { return None; }
    let path = named_children(visibility).into_iter().next()?;
    let path = src.norm(path);
    Some(ContractItem {
        group: "Visibility".into(),
        text: format!("pub(in {path}) restricts access to the module {path} and its descendant modules; the path must name an ancestor of this item. Access still depends on the visibility of the containing modules and other parts of the path. This does not make the item public to other crates."),
        span: src.span(visibility),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn explain_fn(text: &str) -> Option<String> {
        let tree = crate::parse(text).unwrap();
        let node = named_children(tree.root_node()).into_iter().find(|n| n.kind() == "function_item").unwrap();
        let index = LineIndex::new(text);
        explain(Src { text, file: "test.rs", index: &index }, node).map(|item| item.text)
    }

    #[test]
    fn explains_named_ancestor_visibility_without_confusing_other_forms() {
        let text = explain_fn("pub(in crate::path) fn f() {}").unwrap();
        assert!(text.contains("module crate::path and its descendant modules"));
        assert!(text.contains("must name an ancestor"));
        assert!(text.contains("visibility of the containing modules"));
        assert!(explain_fn("pub(crate) fn f() {}").is_none());
        assert!(explain_fn("pub fn f() {}").is_none());
        assert!(explain_fn("fn f() {}").is_none());
    }
}
