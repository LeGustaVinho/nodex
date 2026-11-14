//! Explain `Self` when it occurs in a method signature.

use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, function: Node<'_>) -> Option<ContractItem> {
    if !matches!(function.kind(), "function_item" | "function_signature_item") { return None; }
    let owner = std::iter::successors(function.parent(), |node| node.parent())
        .find(|node| matches!(node.kind(), "impl_item" | "trait_item"))?;
    let mut pending = named_children(function);
    let self_token = loop {
        let node = pending.pop()?;
        if Some(node) == function.child_by_field_name("body") { continue; }
        if matches!(node.kind(), "type_identifier" | "identifier") && src.text(node) == "Self" { break node; }
        pending.extend(named_children(node));
    };
    let meaning = if owner.kind() == "impl_item" {
        let ty = owner.child_by_field_name("type")?;
        format!("In this implementation, Self means {}, the type receiving the implementation.", src.norm(ty))
    } else {
        "In this trait, Self means the concrete type that implements the trait; each implementation can use a different type.".into()
    };
    let result = function.child_by_field_name("return_type").is_some_and(|ty| {
        src.text(ty) == "Self"
    });
    let return_text = if result { " Here -> Self says the function returns that type." } else { "" };
    Some(ContractItem {
        group: "Current type".into(),
        text: format!("{meaning} Capitalized Self is a type name.{return_text} It is different from lowercase self, which is the method's receiver."),
        span: src.span(self_token),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_self_in_impl_and_trait_signatures_but_not_lowercase_receiver() {
        let text = "struct Widget; impl Widget { fn new() -> Self { Self } fn value(&self) {} } trait Build { fn make() -> Self; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "self.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(item) = explain(src, node) { found.push(item.text); }
            pending.extend(named_children(node));
        }
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|text| text.contains("Self means Widget") && text.contains("-> Self")));
        assert!(found.iter().any(|text| text.contains("type that implements the trait")));
    }
}
