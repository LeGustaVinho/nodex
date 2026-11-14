//! Conservative explanation for call syntax resembling a local tuple-struct constructor.

use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    if call.kind() != "call_expression" { return None; }
    let callee = call.child_by_field_name("function")?;
    if callee.kind() != "identifier" { return None; }
    let name = src.text(callee);
    let owner = std::iter::successors(call.parent(), |node| node.parent()).find(|node| {
        node.kind() == "source_file" || (node.kind() == "declaration_list" && node.parent().is_some_and(|parent| parent.kind() == "mod_item"))
    })?;
    let is_tuple_struct = named_children(owner).into_iter().any(|item| {
        item.kind() == "struct_item"
            && item.child_by_field_name("name").is_some_and(|n| src.text(n) == name)
            && item.child_by_field_name("body").is_some_and(|body| body.kind() == "ordered_field_declaration_list")
    });
    if !is_tuple_struct { return None; }
    Some((callee, format!("If {name} here resolves to the tuple struct declared in this module, this calls its generated constructor: each argument supplies the field at the same position, and the result is a {name} value. Rust uses the type name as the constructor; no new keyword is needed. A function or local binding can use the same call syntax, so name resolution determines what this particular call means.")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn tuple_struct_explanation_is_conditional_and_does_not_guess_other_calls() {
        let text = "struct Color(u8,u8,u8); fn f() { let a = Color(255,0,0); let b = Other(1); }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "color.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some((_, description)) = explain(src, node) { found.push(description); }
            pending.extend(named_children(node));
        }
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("If Color here resolves"));
        assert!(found[0].contains("same position"));
        assert!(found[0].contains("name resolution"));
    }
}
