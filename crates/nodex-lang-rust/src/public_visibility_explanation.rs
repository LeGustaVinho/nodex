//! Public visibility on declarations other than functions.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, Src};
use tree_sitter::Node;

/// Functions already have their own explanation. Describe `pub` on other symbols,
/// without confusing it with restricted forms such as `pub(crate)`.
pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if matches!(node.kind(), "function_item" | "function_signature_item") { return None; }
    let visibility = children(node).into_iter().find(|child| child.kind() == "visibility_modifier")?;
    if src.norm(visibility) != "pub" { return None; }
    Some(ContractItem {
        group: "Visibility".into(),
        text: "`pub` makes this declaration public. Code can access it through a path only when the containing modules and other parts of that path are also accessible. A public re-export can provide another accessible path. Public visibility of a type does not automatically make its fields public.".into(),
        span: src.span(visibility),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn explains_public_items_but_not_restricted_or_private_items() {
        let text = "pub mod m {} pub struct S { pub x: i32 } pub(crate) struct C; struct P; pub fn f() {}";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "visibility.rs", index: &index };
        let items = named_children(tree.root_node());
        for node in &items[..2] {
            let result = explain(src, *node).unwrap();
            assert!(result.text.contains("containing modules"));
            assert!(result.text.contains("re-export"));
            assert_eq!(&text[result.span.start_byte as usize..result.span.end_byte as usize], "pub");
        }
        assert!(explain(src, items[2]).is_none());
        assert!(explain(src, items[3]).is_none());
        assert!(explain(src, items[4]).is_none());
        let body = items[1].child_by_field_name("body").unwrap();
        let field = named_children(body)[0];
        assert!(explain(src, field).is_some());
    }
}
