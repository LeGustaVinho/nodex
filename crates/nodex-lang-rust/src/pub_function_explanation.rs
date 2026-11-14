use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Explain unrestricted public visibility without claiming the containing path is public.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let visibility = children(node).into_iter().find(|child| child.kind() == "visibility_modifier")?;
    if src.norm(visibility) != "pub" { return None; }
    let name = node.child_by_field_name("name")?;
    Some(ContractItem {
        group: "Visibility".into(),
        text: format!("pub makes the function {} public: callers can access it if they can reach it through an accessible module path. Public access also depends on the visibility of containing modules; a public re-export can provide another accessible path.", src.norm(name)),
        span: src.span(visibility),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn explains_public_function_but_not_private_or_restricted_visibility() {
        let text = "pub fn open() {} fn private() {} pub(crate) fn within_crate() {}";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "visibility.rs", index: &index };
        let functions = named_children(tree.root_node());
        let item = explain(src, functions[0]).unwrap();
        assert_eq!(item.group, "Visibility");
        assert!(item.text.contains("function open public"));
        assert!(item.text.contains("containing modules"));
        assert!(item.text.contains("re-export"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "pub");
        assert!(explain(src, functions[1]).is_none());
        assert!(explain(src, functions[2]).is_none());
    }
}
