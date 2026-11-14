//! Explanations for paths starting in the immediately enclosing module.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

fn describes(src: Src<'_>, node: Node<'_>) -> Option<String> {
    if !matches!(node.kind(), "scoped_identifier" | "scoped_type_identifier") { return None; }
    let written = src.norm(node);
    let compact = written.replace(char::is_whitespace, "");
    if !compact.starts_with("super::") || compact.starts_with("super::super::") || compact.len() == "super::".len() { return None; }
    Some(format!("In {written}, super means the parent of the current module. The :: separator continues the path from that parent to the named item; this is name lookup, not a parent class method call. Whether the item is accessible depends on its visibility."))
}

pub(crate) fn expression(src: Src<'_>, node: Node<'_>) -> Option<ReadingContent> {
    if node.parent().is_some_and(|parent| matches!(parent.kind(), "scoped_identifier" | "scoped_type_identifier" | "use_declaration")) { return None; }
    Some(ReadingContent::Detail { label: "Look up a name in the parent module".into(), text: describes(src, node)?, borrow: false, definition: None })
}

pub(crate) fn imports(src: Src<'_>, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() { "source_file" => Some(module), "mod_item" => module.child_by_field_name("body"), _ => None };
    let Some(container) = container else { return Vec::new() };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let path = item.child_by_field_name("argument")?;
        Some(ContractItem { group: "Import path".into(), text: format!("{} The use declaration brings this item into the current scope.", describes(src, path)?), span: src.span(path), type_info: None })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;
    #[test]
    fn explains_import_and_expression_without_matching_multiple_super_segments() {
        let text = "mod inner { use super::foo; fn f() { super::foo(); super::super::foo(); } }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "lib.rs", index: &index };
        let mut nodes = vec![tree.root_node()];
        let mut import_count = 0;
        let mut expression_count = 0;
        while let Some(node) = nodes.pop() {
            import_count += imports(src, node).len();
            expression_count += usize::from(expression(src, node).is_some());
            nodes.extend(named_children(node));
        }
        assert_eq!(import_count, 1);
        assert_eq!(expression_count, 1);
    }
}
