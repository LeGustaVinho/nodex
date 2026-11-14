//! Explain a tuple-style enum variant call without assuming name resolution.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Recognize a matching declaration in the enclosing module. The match is only
/// syntactic: aliases, imports, shadowing, and resolution may change its meaning.
pub(crate) fn explain<'t>(src: Src<'_>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    if call.kind() != "call_expression" { return None; }
    let callee = call.child_by_field_name("function")?;
    if callee.kind() != "scoped_identifier" { return None; }
    let enum_name = callee.child_by_field_name("path")?;
    let variant_name = callee.child_by_field_name("name")?;
    if enum_name.kind() != "identifier" || variant_name.kind() != "identifier" { return None; }
    let arguments = call.child_by_field_name("arguments")?;
    let module = std::iter::successors(call.parent(), Node::parent).find(|node| {
        node.kind() == "source_file" || (node.kind() == "declaration_list" && node.parent().is_some_and(|p| p.kind() == "mod_item"))
    })?;
    let en = src.norm(enum_name);
    let var = src.norm(variant_name);
    let declared = named_children(module).into_iter().any(|item| {
        item.kind() == "enum_item"
            && item.child_by_field_name("name").is_some_and(|n| src.norm(n) == en)
            && item.child_by_field_name("body").is_some_and(|body| named_children(body).into_iter().any(|variant| {
                variant.kind() == "enum_variant"
                    && variant.child_by_field_name("name").is_some_and(|n| src.norm(n) == var)
                    && variant.child_by_field_name("body").is_some_and(|b| b.kind() == "ordered_field_declaration_list")
            }))
    });
    if !declared { return None; }
    Some((arguments, format!("If {en}::{var} resolves to the matching variant declared in this module, this constructs a {en} value in its {var} variant. Each argument supplies the value for the field in the same position: for example, the first argument supplies field 0 and the second supplies field 1. The variant records which case the value is; Rust uses the variant path as its constructor without a new keyword. Name resolution determines whether this path instead refers to something else.")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn only_explains_a_syntactically_matching_tuple_variant() {
        let text = "enum Message { Quit, Move(i32, i32), Named { x: i32 } } fn f() { Message::Move(1, 2); Message::Quit(); Message::Named(1); Other::Move(1, 2); }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "test.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = nodes.pop() {
            if let Some((arguments, description)) = explain(src, node) { found.push((src.text(arguments).to_string(), description)); }
            nodes.extend(named_children(node));
        }
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "(1, 2)");
        assert!(found[0].1.contains("If Message::Move resolves"));
        assert!(found[0].1.contains("same position"));
        assert!(found[0].1.contains("Name resolution"));
    }
}
