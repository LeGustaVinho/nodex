//! Beginner-facing explanation of a public re-export on its containing module.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|declaration| {
        if declaration.kind() != "use_declaration" { return None; }
        let visibility = named_children(declaration).into_iter().find(|child| child.kind() == "visibility_modifier")?;
        if src.norm(visibility) != "pub" { return None; }
        let path = declaration.child_by_field_name("argument")?;
        if path.kind() != "scoped_identifier" { return None; }
        let parent = path.child_by_field_name("path")?;
        let name = path.child_by_field_name("name")?;
        if parent.kind() != "identifier" || name.kind() != "identifier" { return None; }
        let (parent, name) = (src.norm(parent), src.norm(name));
        Some(ContractItem {
            group: "Public re-export".into(),
            text: format!("Bring {parent}::{name} into this module under the name {name}, and make that name public from this module: other code can use this module's path to reach the same item. This is called a re-export; it does not create a new item or execute it. Outside access also depends on the visibility of the containing modules and the original item."),
            span: src.span(declaration),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn only_public_simple_paths_are_explained_in_their_module() {
        let text = "pub use a::b; use c::d; pub(crate) use e::f; mod nested { pub use g::h; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let explanations = explain(src, root);
        assert_eq!(explanations.len(), 1);
        assert!(explanations[0].text.contains("a::b"));
        assert!(explanations[0].text.contains("same item"));
        let nested = named_children(root).into_iter().find(|node| node.kind() == "mod_item").unwrap();
        assert_eq!(explain(src, nested).len(), 1);
        assert!(explain(src, nested)[0].text.contains("g::h"));
    }
}
