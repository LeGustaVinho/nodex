//! Explanation of inline Rust module declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "mod_item" || node.child_by_field_name("body").is_none() {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    let name = src.norm(name);
    Some(ContractItem {
        group: "Module".into(),
        text: format!("`mod` declares a module named {name}. The braces contain its declarations directly in this source file. A module groups names and forms part of their paths: for example, an item called helper inside it can be referred to as {name}::helper when visibility permits. Items are private by default; `pub` can make an item accessible outside its module, subject to its enclosing modules' visibility."),
        span: src.span(node),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    fn explanation(text: &str) -> Option<ContractItem> {
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let item = tree.root_node().named_child(0).unwrap();
        explain(src, item)
    }

    #[test]
    fn explains_inline_modules_only() {
        let item = explanation("mod utils { fn helper() {} }").unwrap();
        assert!(item.text.contains("utils::helper"));
        assert!(item.text.contains("private by default"));
        assert!(explanation("mod utils;").is_none());
        assert!(explanation("fn utils() {}").is_none());
    }
}
