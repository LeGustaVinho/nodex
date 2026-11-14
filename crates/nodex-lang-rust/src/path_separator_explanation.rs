//! Explain Rust's path separator without guessing what a name resolves to.
use nodex_lang_api::syntax::Src;
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if !matches!(node.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    // One explanation for an entire path, not one for each nested prefix.
    if node.parent().is_some_and(|p| matches!(p.kind(), "scoped_identifier" | "scoped_type_identifier")) { return None; }
    let prefix = node.child_by_field_name("path")?;
    let name = node.child_by_field_name("name")?;
    Some(format!("`::` joins parts of a path. Here {} names an item through {}. If the left side denotes a type, the item can be an associated function, constant, type, or enum variant; the left side can also denote a module or another path. The exact meaning depends on name resolution and context.", src.norm(name), src.norm(prefix)))
}

pub(crate) const TITLE: &str = "Follow a Rust path";

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{kit::LineIndex, syntax::named_children};

    fn explanations(text: &str) -> Vec<String> {
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "paths.rs", index: &index };
        let mut todo = vec![tree.root_node()];
        let mut found = Vec::new();
        while let Some(node) = todo.pop() {
            if let Some(text) = explain(src, node) { found.push(text); }
            todo.extend(named_children(node));
        }
        found
    }

    #[test]
    fn explains_paths_without_guessing_resolution() {
        let found = explanations("fn f() { let x = Type::Item; let y = module::item; }");
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|text| text.contains("Item names an item through Type")));
        assert!(found.iter().all(|text| text.contains("depends on name resolution")));
        assert_eq!(explanations("fn f() { let x = a::b::C; }").len(), 1);
        assert!(explanations("fn f() { let x = value; }").is_empty());
    }
}
