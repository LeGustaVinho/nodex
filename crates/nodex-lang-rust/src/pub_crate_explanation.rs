//! Explanation of crate-restricted visibility.

use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, item: Node<'_>) -> Option<ContractItem> {
    let visibility = named_children(item).into_iter().find(|n| n.kind() == "visibility_modifier")?;
    let tokens = children(visibility);
    if !tokens.iter().any(|n| n.kind() == "crate") || tokens.iter().any(|n| n.kind() == "in") {
        return None;
    }
    Some(ContractItem {
        group: "Visibility".into(),
        text: "pub(crate) makes this item accessible throughout the current crate, subject to the visibility of its containing modules. Other crates cannot access it directly. A crate is a compilation unit, such as one library or executable; it is not necessarily the entire package.".into(),
        span: src.span(visibility),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn read(text: &str) -> Option<String> {
        let tree = crate::parse(text).unwrap();
        let node = named_children(tree.root_node()).into_iter().find(|node| node.kind() == "function_item").unwrap();
        let index = LineIndex::new(text);
        explain(Src { text, file: "test.rs", index: &index }, node).map(|item| item.text)
    }

    #[test]
    fn only_explains_crate_restricted_visibility() {
        let text = read("pub(crate) fn f() {}").unwrap();
        assert!(text.contains("current crate"));
        assert!(text.contains("containing modules"));
        assert!(read("pub fn f() {}").is_none());
        assert!(read("pub(in crate::inner) fn f() {}").is_none());
        assert!(read("fn f() {}").is_none());
    }
}
