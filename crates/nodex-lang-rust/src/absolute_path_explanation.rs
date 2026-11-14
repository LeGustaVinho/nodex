use nodex_ir::{reading::{ContractItem, ReadingContent}, Span};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn path_description(src: Src<'_>, path: Node<'_>) -> Option<String> {
    if !matches!(path.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    // Explain the complete path once, not each nested prefix.
    if path.parent().is_some_and(|parent| matches!(parent.kind(), "scoped_identifier" | "scoped_type_identifier")) { return None; }
    let written = src.norm(path);
    let rest = written.strip_prefix("::")?;
    let (crate_name, remainder) = rest.split_once("::")?;
    if crate_name.is_empty() || remainder.is_empty() { return None; }
    Some(format!("{written} is a path: :: separates its names. In Rust 2018 and later, the leading :: starts from the external crate namespace; {crate_name} names an available external crate and {remainder} selects an item within it. In Rust 2015, a leading :: instead starts from the current crate root. Which item is accessible depends on the edition, dependencies and visibility."))
}

/// Detail for an absolute path used in an expression or type.
pub(crate) fn explain_path(src: Src<'_>, node: Node<'_>) -> Option<(Span, ReadingContent)> {
    if std::iter::successors(node.parent(), Node::parent).any(|ancestor| ancestor.kind() == "use_declaration") { return None; }
    let text = path_description(src, node)?;
    Some((src.span(node), ReadingContent::Detail {
        label: "Follow an absolute path".into(), text, definition: None, borrow: false,
    }))
}

/// Import declarations belong to the contract of their enclosing module.
pub(crate) fn explain_imports(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => module,
        "mod_item" => match module.child_by_field_name("body") { Some(body) => body, None => return Vec::new() },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let path = item.child_by_field_name("argument")?;
        let text = path_description(src, path)?;
        Some(ContractItem { group: "Absolute import path".into(), text, span: src.span(path), type_info: None })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_absolute_import_and_expression_once() {
        let text = "use ::crate_name::foo; fn f() { ::crate_name::foo(); crate_name::foo(); }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let root = tree.root_node();
        let imports = explain_imports(src, root);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].text.contains("external crate namespace"));
        assert!(imports[0].text.contains("Rust 2015"));
        let mut nodes = vec![root];
        let mut explanations = Vec::new();
        while let Some(node) = nodes.pop() {
            if let Some(detail) = explain_path(src, node) { explanations.push(detail); }
            nodes.extend(named_children(node));
        }
        assert_eq!(explanations.len(), 1);
        let (span, ReadingContent::Detail { text: description, .. }) = &explanations[0] else { panic!("expected path detail") };
        assert_eq!(&text[span.start_byte as usize..span.end_byte as usize], "::crate_name::foo");
        assert!(description.contains("crate_name names an available external crate"));
    }
}
