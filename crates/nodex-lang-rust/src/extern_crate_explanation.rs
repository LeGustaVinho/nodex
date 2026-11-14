use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Explain direct extern-crate declarations in a module, without descending into nested modules.
pub(crate) fn explain_module(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => module,
        "mod_item" => match module.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|node| {
        if node.kind() != "extern_crate_declaration" { return None; }
        let name = node.child_by_field_name("name")?;
        let crate_name = src.norm(name);
        let alias = node.child_by_field_name("alias").map(|alias| src.norm(alias));
        let intro = if crate_name == "self" {
            "This declaration refers to the current crate".to_owned()
        } else {
            format!("This declaration brings the crate {crate_name} into scope")
        };
        let alias_text = alias.map(|alias| format!(" under the name {alias}")).unwrap_or_default();
        Some(ContractItem {
            group: "External crate".into(),
            text: format!("{intro}{alias_text}. A crate is a Rust compilation unit (such as a library). This older explicit declaration is usually unnecessary in Rust 2018 and later because dependencies can be used directly by name; some specialized cases still use it. The dependency must be available to the build."),
            span: src.span(node),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_extern_crate_in_the_correct_module() {
        let text = "extern crate foo; mod inner { extern crate bar as baz; } fn f() {}";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let root_items = explain_module(src, root);
        assert_eq!(root_items.len(), 1);
        assert!(root_items[0].text.contains("crate foo into scope"));
        assert!(root_items[0].text.contains("usually unnecessary"));
        assert_eq!(&text[root_items[0].span.start_byte as usize..root_items[0].span.end_byte as usize], "extern crate foo;");
        let inner = named_children(root).into_iter().find(|node| node.kind() == "mod_item").unwrap();
        let nested = explain_module(src, inner);
        assert_eq!(nested.len(), 1);
        assert!(nested[0].text.contains("crate bar into scope"));
        assert!(nested[0].text.contains("under the name baz"));
    }
}
