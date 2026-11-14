//! Explain record syntax as a union initializer only when a matching declaration is visible.
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Return a field anchor and an explicitly conditional explanation. Syntax alone does not
/// resolve a name, so even a nearby declaration does not prove which item the expression uses.
pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, String)> {
    if node.kind() != "struct_expression" { return None; }
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == "assignment_expression" {
            if ancestor.child_by_field_name("left").is_some_and(|left| left.start_byte() <= node.start_byte() && left.end_byte() >= node.end_byte()) { return None; }
            break;
        }
        if matches!(ancestor.kind(), "block" | "function_item") { break; }
    }
    let name = node.child_by_field_name("name")?;
    if name.kind() != "type_identifier" && name.kind() != "identifier" { return None; }
    let body = node.child_by_field_name("body")?;
    let fields = named_children(body);
    if fields.len() != 1 || fields[0].kind() != "field_initializer" { return None; }
    let field = fields[0].child_by_field_name("field")?;
    let value = fields[0].child_by_field_name("value")?;
    let owner = std::iter::successors(node.parent(), Node::parent).find(|ancestor| {
        ancestor.kind() == "source_file" || (ancestor.kind() == "declaration_list" && ancestor.parent().is_some_and(|parent| parent.kind() == "mod_item"))
    })?;
    let name_text = src.text(name);
    let declared = named_children(owner).into_iter().any(|item| item.kind() == "union_item" && item.child_by_field_name("name").is_some_and(|n| src.text(n) == name_text));
    if !declared { return None; }
    Some((field, format!("If {name_text} here resolves to the union declared in this module, this creates a {name_text} value by initializing its {} field with {}. A union initializer specifies exactly one field: its fields share the same storage, so this chooses how the initial bits are written. Reading a union field later is unsafe and requires the caller to uphold its validity rules. The name may resolve differently; name resolution determines which declaration this expression uses.", src.norm(field), src.norm(value))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn recognizes_only_one_field_with_a_local_union_declaration() {
        let text = "union Bits { i: u32, f: f32 } struct Other { i: u32 } fn f() { let x = Bits { i: 1 }; let y = Other { i: 1 }; Bits { i: z } = x; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut pending = vec![tree.root_node()];
        let mut results = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some((field, text)) = explain(src, node) { results.push((src.text(field).to_owned(), text)); }
            pending.extend(named_children(node));
        }
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "i");
        assert!(results[0].1.contains("If Bits here resolves"));
        assert!(results[0].1.contains("exactly one field"));
        assert!(results[0].1.contains("unsafe"));
    }
}
