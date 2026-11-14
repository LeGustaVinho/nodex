//! Explanations for paths that climb two module levels.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describes(src: Src<'_>, path: Node<'_>) -> Option<String> {
    if path.kind() != "scoped_identifier" { return None; }
    let text = src.norm(path);
    let compact = text.replace(char::is_whitespace, "");
    if !compact.starts_with("super::super::") || compact.len() == "super::super::".len() { return None; }
    Some(format!("In {text}, each `super` means the parent module. Starting from the current module, go up two module levels, then look up the remaining path there. `::` separates the path segments. This is a name lookup, not a call or a value by itself; what happens next depends on how the item is used. The path must resolve and satisfy visibility rules."))
}

/// Attach a source-linked explanation to an ordinary expression path.
pub(crate) fn expression(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.parent().is_some_and(|parent| matches!(parent.kind(), "scoped_identifier" | "use_declaration")) {
        return None;
    }
    Some(ReadingContent::Detail {
        label: "Look up a name two modules above".into(),
        text: describes(src, node)?,
        definition: None,
        borrow: false,
    })
}

/// Put imports on their containing module's visible contract.
pub(crate) fn imports(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => Some(module),
        "mod_item" => module.child_by_field_name("body"),
        _ => None,
    };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let path = item.child_by_field_name("argument")?;
        Some(ContractItem {
            group: "Import path".into(),
            text: format!("{} The `use` declaration brings the named item into this module's scope.", describes(src, path)?),
            span: src.span(path),
            type_info: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_import_and_expression_without_matching_one_super() {
        let text = "mod outer { mod inner { use super::super::foo; fn f() { super::super::foo(); super::foo(); } } }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut imports_found = 0;
        let mut expressions_found = 0;
        while let Some(node) = nodes.pop() {
            imports_found += imports(src, node).len();
            expressions_found += usize::from(expression(src, node).is_some());
            nodes.extend(named_children(node));
        }
        assert_eq!(imports_found, 1);
        assert_eq!(expressions_found, 1);
    }
}
