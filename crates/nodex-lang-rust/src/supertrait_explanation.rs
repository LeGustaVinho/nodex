//! Explain the requirements introduced by a trait's `:` bounds.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "trait_item" { return None; }
    let name = node.child_by_field_name("name")?;
    let bounds = node.child_by_field_name("bounds")?;
    let written = src.norm(bounds);
    let requirements = written.trim().strip_prefix(':')?.trim();
    if requirements.is_empty() { return None; }
    Some(ContractItem {
        group: "Trait requirements".into(),
        text: format!(
            "The colon after {} introduces requirements for this trait: {}. A type implementing {} must also satisfy every listed requirement. A requirement naming another trait is a supertrait: for example, Child: Parent requires a type implementing Child to implement Parent too. These requirements constrain implementations; they do not add stored fields or automatically provide the required implementation.",
            src.norm(name), requirements, src.norm(name),
        ),
        span: src.span(bounds),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    #[test]
    fn explains_trait_bounds_without_treating_other_items_as_traits() {
        let text = "trait Child: Parent {} trait Both: Send + Sync {} trait Plain {} struct S;";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let nodes = named_children(tree.root_node());
        let child = explain(src, nodes[0]).unwrap();
        assert!(child.text.contains("Child: Parent requires a type implementing Child to implement Parent"));
        assert!(child.text.contains("do not add stored fields"));
        assert_eq!(&text[child.span.start_byte as usize..child.span.end_byte as usize], ": Parent");
        assert!(explain(src, nodes[1]).unwrap().text.contains("Send + Sync"));
        assert!(explain(src, nodes[2]).is_none());
        assert!(explain(src, nodes[3]).is_none());
    }
}
