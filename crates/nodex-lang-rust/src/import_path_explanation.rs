//! Explanations of simple two-segment imports on their containing module.
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
        if declaration.kind() != "use_declaration" || src.norm(declaration).starts_with("pub") {
            return None;
        }
        let path = declaration.child_by_field_name("argument")?;
        if path.kind() != "scoped_identifier" { return None }
        let parent = path.child_by_field_name("path")?;
        let name = path.child_by_field_name("name")?;
        if parent.kind() != "identifier" || name.kind() != "identifier" { return None }
        let (parent, name) = (src.norm(parent), src.norm(name));
        Some(ContractItem {
            group: "Import".into(),
            text: format!("Bring {name} from {parent} into this scope, so code here can write {name} instead of {parent}::{name}. The :: separator follows a path to a name; use does not run the imported item."),
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
    fn explains_import_in_its_own_scope() {
        let text = "use std::fmt; mod inner { use other::thing; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let imports = explain(src, root);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].text.contains("fmt from std"));
        assert!(imports[0].text.contains("fmt instead of std::fmt"));
        let inner = named_children(root).into_iter().find(|n| n.kind() == "mod_item").unwrap();
        assert!(explain(src, inner)[0].text.contains("thing from other"));
    }
}
