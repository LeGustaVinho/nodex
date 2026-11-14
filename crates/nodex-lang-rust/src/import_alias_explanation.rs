use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe direct renamed imports in the contract of their containing module.
pub(crate) fn explain(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" || src.norm(item).starts_with("pub") { return None; }
        let argument = item.child_by_field_name("argument")?;
        if argument.kind() != "use_as_clause" { return None; }
        let path = argument.child_by_field_name("path")?;
        let alias = argument.child_by_field_name("alias")?;
        let alias = src.norm(alias);
        if alias == "_" { return None; }
        Some(ContractItem {
            group: "Import alias".into(),
            text: format!("Bring {} into this module's scope under the local name {alias}. The as keyword gives the imported item a different name here; it does not rename the original item or run it. The :: symbols separate path segments.", src.norm(path)),
            span: src.span(item),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_alias_in_its_own_module_without_conflating_other_imports() {
        let text = "use a::b as alias; use x::y; mod inner { use c::d as local; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let imports = explain(src, root);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].text.contains("a::b into this module's scope under the local name alias"));
        assert!(imports[0].text.contains("does not rename the original item"));
        let inner = named_children(root).into_iter().find(|n| n.kind() == "mod_item").unwrap();
        assert!(explain(src, inner)[0].text.contains("c::d into this module's scope under the local name local"));
    }
}
