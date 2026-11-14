//! Explanation of visibility restricted to the parent module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    let visibility = children(node).into_iter().find(|child| child.kind() == "visibility_modifier")?;
    let written = src.norm(visibility);
    if written.chars().filter(|c| !c.is_whitespace()).collect::<String>() != "pub(super)" {
        return None;
    }
    let name = node.child_by_field_name("name").map(|name| src.norm(name)).unwrap_or_else(|| "This item".into());
    Some(ContractItem {
        group: "Visibility".into(),
        text: format!("pub(super) makes {name} accessible within its parent module and that parent's descendant modules. Here super means the module immediately containing the module where this item is declared. Access still requires an accessible path through any intervening modules; this restriction does not make the item public everywhere."),
        span: src.span(visibility),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn explains_parent_restricted_visibility_only() {
        let text = "pub(super) fn f() {} pub(crate) fn g() {} fn h() {}";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "visibility.rs", index: &index };
        let declarations = named_children(tree.root_node());
        let item = explain(src, declarations[0]).unwrap();
        assert_eq!(item.group, "Visibility");
        assert!(item.text.contains("f accessible within its parent module"));
        assert!(item.text.contains("descendant modules"));
        assert!(item.text.contains("accessible path"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "pub(super)");
        assert!(explain(src, declarations[1]).is_none());
        assert!(explain(src, declarations[2]).is_none());
    }
}
