//! Explanation of a direct wildcard import in a Rust module.

use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, declaration: Node<'_>) -> Option<ContractItem> {
    if declaration.kind() != "use_declaration" { return None; }
    let argument = declaration.child_by_field_name("argument")?;
    if argument.kind() != "use_wildcard" { return None; }
    let path = named_children(argument).into_iter().next()?;
    let module = src.norm(path);
    Some(ContractItem {
        group: "Wildcard import".into(),
        text: format!("use {module}::* brings the names that are accessible from {module} directly into this scope, so they can be written without the {module}:: prefix. The * means a wildcard, not multiplication. It does not make private items accessible; which names are available depends on visibility and name resolution."),
        span: src.span(declaration),
        type_info: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn explains_wildcard_but_not_named_import() {
        let text = "use a::*; use a::b;";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "imports.rs", index: &index };
        let imports = named_children(tree.root_node());
        let item = explain(src, imports[0]).unwrap();
        assert!(item.text.contains("a::*"));
        assert!(item.text.contains("private items"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "use a::*;");
        assert!(explain(src, imports[1]).is_none());
    }
}
