use nodex_ir::{Language, reading::ContractItem, types::{TypeInfo, TypeOrigin, syntax_shape}};
use nodex_lang_api::syntax::{Src, children};
use tree_sitter::Node;

/// Describe an immutable static item at its declaration.
pub(crate) fn explain(src: Src, node: Node) -> Option<ContractItem> {
    if node.kind() != "static_item" || children(node).iter().any(|n| n.kind() == "mutable_specifier") {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    let value = node.child_by_field_name("value")?;
    let native = src.norm(ty);
    Some(ContractItem {
        group: "Static value".into(),
        text: format!(
            "{} names a statically allocated value of type {}. Its initializer {} is evaluated at compile time. The value remains available for the entire program, and uses refer to the same static storage. This declaration cannot be reassigned; its type may still permit controlled interior mutation.",
            src.norm(name), native, src.norm(value),
        ),
        span: src.span(node),
        type_info: Some(TypeInfo::new(syntax_shape(Language::Rust, &native), TypeOrigin::Declared, Some(native))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_static_storage_and_skips_mutable_static_and_const() {
        let text = "static X: i32 = 10; static mut Y: i32 = 11; const Z: i32 = 12;";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "static.rs", index: &index };
        let items = nodex_lang_api::syntax::named_children(tree.root_node());
        let item = explain(src, items[0]).unwrap();
        assert!(item.text.contains("X names a statically allocated value of type i32"));
        assert!(item.text.contains("initializer 10 is evaluated at compile time"));
        assert!(item.text.contains("same static storage"));
        assert!(item.text.contains("interior mutation"));
        assert!(explain(src, items[1]).is_none());
        assert!(explain(src, items[2]).is_none());
    }
}
